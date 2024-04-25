use crate::utils::{advance_chain, setup};
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::test]
async fn can_sync() -> eyre::Result<()> {
    reth_tracing::init_test_tracing();

    let (mut nodes, _tasks, wallet) = setup(3).await?;
    let wallet = Arc::new(Mutex::new(wallet));

    let third_node = nodes.pop().unwrap();
    let mut second_node = nodes.pop().unwrap();
    let mut first_node = nodes.pop().unwrap();

    let tip: usize = 90;
    let tip_index: usize = tip - 1;

    // On first node, create a chain up to block number 90a
    let canonical_payload_chain = advance_chain(tip, &mut first_node, wallet.clone()).await?;
    let canonical_chain =
        canonical_payload_chain.iter().map(|p| p.0.block().hash()).collect::<Vec<_>>();

    // On second node, sync optimistically up to block number 87a
    second_node.engine_api.update_optimistic_forkchoice(canonical_chain[tip_index - 1]).await?;
    second_node.wait_block(tip as u64 - 1, canonical_chain[tip_index - 1], true).await?;

    // On third node, sync optimistically up to block number 90a
    third_node.engine_api.update_optimistic_forkchoice(canonical_chain[tip_index]).await?;
    third_node.wait_block(tip as u64, canonical_chain[tip_index], true).await?;

    let reorg_depth = 1usize;

    //  On second node, create a side chain: 88b ->  89b -> 90b
    wallet.lock().await.inner_nonce -= reorg_depth as u64;
    second_node.payload.timestamp = first_node.payload.timestamp - reorg_depth as u64; // TODO: probably want to make it node agnostic
    let side_payload_chain = advance_chain(reorg_depth, &mut second_node, wallet.clone()).await?;
    let side_chain = side_payload_chain.iter().map(|p| p.0.block().hash()).collect::<Vec<_>>();

    // On third node, cause a re-org
    assert!(side_chain[reorg_depth - 1] != canonical_chain[tip_index]);
    third_node.engine_api.update_optimistic_forkchoice(dbg!(side_chain[reorg_depth - 1])).await?;
    third_node
        .wait_block(
            side_payload_chain[reorg_depth - 1].0.block().number,
            side_chain[reorg_depth - 1],
            true,
        )
        .await?;

    Ok(())
}
