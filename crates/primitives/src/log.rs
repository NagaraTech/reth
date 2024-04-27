use crate::{Address, Bloom, Bytes, B256};

/// Re-export `Log` from `alloy_primitives`.
pub use alloy_primitives::Log;
use alloy_rlp::{RlpDecodable, RlpEncodable};
use reth_codecs::{main_codec, Compact};

/// Calculate receipt logs bloom.
pub fn logs_bloom<'a, It>(logs: It) -> Bloom
where
    It: IntoIterator<Item = &'a Log>,
{
    let mut bloom = Bloom::ZERO;
    for log in logs {
        bloom.m3_2048(log.address.as_slice());
        for topic in log.topics() {
            bloom.m3_2048(topic.as_slice());
        }
    }
    bloom
}

/// This type is kept for compatibility tests after the codec support was added to
/// alloy-primitives Log type natively
///
/// See <https://github.com/ethereum/go-ethereum/blob/4253030ef67a2af2e59bbd1fd90a4c1e75939b9f/core/types/log.go#L30-L36>
#[main_codec(rlp)]
#[derive(Clone, Debug, PartialEq, Eq, RlpDecodable, RlpEncodable, Default)]
pub struct ConsensusLog {
    /// Contract that emitted this log.
    pub address: Address,
    /// Topics of the log. The number of logs depend on what `LOG` opcode is used.
    #[cfg_attr(
        any(test, feature = "arbitrary"),
        proptest(
            strategy = "proptest::collection::vec(proptest::arbitrary::any::<B256>(), 0..=5)"
        )
    )]
    pub topics: Vec<B256>,
    /// Arbitrary length data.
    pub data: Bytes,
}

impl From<Log> for ConsensusLog {
    fn from(mut log: Log) -> Self {
        Self {
            address: log.address,
            topics: std::mem::take(log.data.topics_mut_unchecked()),
            data: log.data.data,
        }
    }
}

impl From<ConsensusLog> for Log {
    fn from(log: ConsensusLog) -> Log {
        Log::new_unchecked(log.address, log.topics, log.data)
    }
}

#[cfg(test)]
mod tests {
    use proptest::proptest;

    use super::*;

    proptest! {
        #[test]
        fn test_roundtrip_conversion_between_log_and_alloy_log(log: ConsensusLog) {
            // Convert log to buffer and then create alloy_log from buffer and compare
            let mut compacted_log = Vec::<u8>::new();
            let len = log.clone().to_compact(&mut compacted_log);

            let alloy_log = Log::from_compact(&compacted_log, len).0;
            assert_eq!(log, alloy_log.into());

            // Create alloy_log from log and then convert it to buffer and compare compacted_alloy_log and compacted_log
            let alloy_log = Log::new_unchecked(log.address, log.topics, log.data);
            let mut compacted_alloy_log = Vec::<u8>::new();
            let alloy_len = alloy_log.to_compact(&mut compacted_alloy_log);
            assert_eq!(len, alloy_len);
            assert_eq!(compacted_log, compacted_alloy_log);
        }
    }
}
