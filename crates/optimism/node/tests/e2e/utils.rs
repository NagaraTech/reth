use reth::{
    args::{DiscoveryArgs, NetworkArgs, RpcServerArgs},
    rpc::types::engine::PayloadAttributes,
    tasks::{TaskExecutor, TaskManager},
};
use reth_e2e_test_utils::{node::NodeHelper, wallet::Wallet};
use reth_node_builder::{NodeBuilder, NodeConfig, NodeHandle};
use reth_node_optimism::{OptimismBuiltPayload, OptimismNode, OptimismPayloadBuilderAttributes};
use reth_payload_builder::EthPayloadBuilderAttributes;
use reth_primitives::{Address, ChainSpecBuilder, Genesis, B256, BASE_MAINNET};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{span, Level};

pub(crate) async fn setup(
    num_nodes: usize,
) -> eyre::Result<(Vec<OpNode>, TaskManager, TaskExecutor, Wallet)> {
    let tasks = TaskManager::current();
    let exec = tasks.executor();

    let genesis: Genesis = serde_json::from_str(include_str!("../assets/genesis.json")).unwrap();
    let chain_spec = Arc::new(
        ChainSpecBuilder::default()
            .chain(BASE_MAINNET.chain)
            .genesis(genesis)
            .ecotone_activated()
            .build(),
    );
    let chain_id = chain_spec.chain.into();

    let network_config = NetworkArgs {
        discovery: DiscoveryArgs { disable_discovery: true, ..DiscoveryArgs::default() },
        ..NetworkArgs::default()
    };

    // Create nodes and peer them
    let mut nodes: Vec<OpNode> = Vec::with_capacity(num_nodes);
    for idx in 0..num_nodes {
        let node_config = NodeConfig::test()
            .with_chain(chain_spec.clone())
            .with_network(network_config.clone())
            .with_unused_ports()
            .with_rpc(RpcServerArgs::default().with_unused_ports().with_http());

        let mut node =  node(node_config, exec.clone(), idx + 1).await?;
        
        // Connect each node in a chain.
        if let Some(previous_node) = nodes.last_mut() {
            previous_node.connect(&mut node).await;
        }

        // Connect last node with the first if there are more than two
        if idx + 1 == num_nodes && num_nodes > 2 {
            if let Some(first_node) = nodes.first_mut() {
                node.connect(first_node).await;
            }
        }

        nodes.push(node);
    }

    Ok((nodes, tasks, exec, Wallet::default().with_chain_id(chain_id)))
}

pub(crate) async fn node(
    node_config: NodeConfig,
    exec: TaskExecutor,
    id: usize,
) -> eyre::Result<OpNode> {
    let span = span!(Level::INFO, "node", id);
    let _enter = span.enter();
    let NodeHandle { node, node_exit_future: _ } = NodeBuilder::new(node_config.clone())
        .testing_node(exec.clone())
        .node(OptimismNode::default())
        .launch()
        .await?;

    NodeHelper::new(node).await
}

pub(crate) async fn advance_chain(
    length: usize,
    node: &mut OpNode,
    wallet: Arc<Mutex<Wallet>>,
) -> eyre::Result<Vec<(OptimismBuiltPayload, OptimismPayloadBuilderAttributes)>> {
    node.advance(
        length as u64,
        || {
            let wallet = wallet.clone();
            Box::pin(async move { wallet.lock().await.optimism_l1_block_info_tx().await })
        },
        optimism_payload_attributes,
    )
    .await
}

/// Helper function to create a new eth payload attributes
pub(crate) fn optimism_payload_attributes(timestamp: u64) -> OptimismPayloadBuilderAttributes {
    let attributes = PayloadAttributes {
        timestamp,
        prev_randao: B256::ZERO,
        suggested_fee_recipient: Address::ZERO,
        withdrawals: Some(vec![]),
        parent_beacon_block_root: Some(B256::ZERO),
    };

    OptimismPayloadBuilderAttributes {
        payload_attributes: EthPayloadBuilderAttributes::new(B256::ZERO, attributes),
        transactions: vec![],
        no_tx_pool: false,
        gas_limit: Some(30_000_000),
    }
}

// Type alias
type OpNode = NodeHelper<
    reth_node_api::FullNodeComponentsAdapter<
        reth_node_api::FullNodeTypesAdapter<
            OptimismNode,
            Arc<reth_db::test_utils::TempDatabase<reth_db::DatabaseEnv>>,
            reth_provider::providers::BlockchainProvider<
                Arc<reth_db::test_utils::TempDatabase<reth_db::DatabaseEnv>>,
                reth::blockchain_tree::ShareableBlockchainTree<
                    Arc<reth_db::test_utils::TempDatabase<reth_db::DatabaseEnv>>,
                    reth_revm::EvmProcessorFactory<reth_node_optimism::OptimismEvmConfig>,
                >,
            >,
        >,
        reth_transaction_pool::Pool<
            reth_transaction_pool::TransactionValidationTaskExecutor<
                reth_node_optimism::txpool::OpTransactionValidator<
                    reth_provider::providers::BlockchainProvider<
                        Arc<reth_db::test_utils::TempDatabase<reth_db::DatabaseEnv>>,
                        reth::blockchain_tree::ShareableBlockchainTree<
                            Arc<reth_db::test_utils::TempDatabase<reth_db::DatabaseEnv>>,
                            reth_revm::EvmProcessorFactory<reth_node_optimism::OptimismEvmConfig>,
                        >,
                    >,
                    reth_transaction_pool::EthPooledTransaction,
                >,
            >,
            reth_transaction_pool::CoinbaseTipOrdering<reth_transaction_pool::EthPooledTransaction>,
            reth_transaction_pool::blobstore::DiskFileBlobStore,
        >,
    >,
>;
