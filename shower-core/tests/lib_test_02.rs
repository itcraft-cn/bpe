mod test_log;
mod test_timestamp;

use test_log::init_logger;
use test_timestamp::special_timestamp;

const TM_TEST_SIZE: usize = 10000;

#[test]
fn test_timestamp() {
    init_logger();
    let timestamp = special_timestamp(2023, 10, 12, 12, 46, 3);
    for i in 0..TM_TEST_SIZE {
        log::info!("timestamp: {}", timestamp + i as u64 * 1000);
    }
}
