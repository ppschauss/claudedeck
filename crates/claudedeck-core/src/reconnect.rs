use std::time::Duration;

pub fn backoff_schedule() -> impl Iterator<Item = Duration> {
    [3u64, 6, 12]
        .into_iter()
        .chain(std::iter::repeat(30))
        .map(Duration::from_secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_ist_3_6_12_dann_dauerhaft_30() {
        let v: Vec<Duration> = backoff_schedule().take(6).collect();
        assert_eq!(
            v,
            vec![
                Duration::from_secs(3),
                Duration::from_secs(6),
                Duration::from_secs(12),
                Duration::from_secs(30),
                Duration::from_secs(30),
                Duration::from_secs(30),
            ]
        );
    }
}
