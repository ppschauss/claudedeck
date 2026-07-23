use std::time::Duration;

pub fn backoff_schedule() -> impl Iterator<Item = Duration> {
    [3u64, 6, 12]
        .into_iter()
        .chain(std::iter::repeat(30))
        .map(Duration::from_secs)
}

/// Liefert die Backoff-Wartezeit für den `attempt`-ten Reconnect-Versuch (1-basiert: 3s, 6s,
/// 12s, danach dauerhaft 30s) — dieselbe Folge wie [`backoff_schedule`], aber als reine
/// Einzelwert-Funktion statt eines Iterators. M4/M5-Task-6-Ergänzung: der Reconnect-Supervisor
/// (`src-tauri/src/reconnect_supervisor.rs`) braucht die Wartezeit pro Versuch einzeln, ohne
/// einen `impl Iterator` über `.await`-Punkte einer `tokio::select!`-Schleife hinweg am Leben
/// halten zu müssen — ein einfacher `attempt: u32`-Zähler genügt. `attempt == 0` wird wie `1`
/// behandelt (kein Panik/Sonderfall für einen versehentlich nicht hochgezählten Aufrufer).
pub fn attempt_delay(attempt: u32) -> Duration {
    match attempt {
        0 | 1 => Duration::from_secs(3),
        2 => Duration::from_secs(6),
        3 => Duration::from_secs(12),
        _ => Duration::from_secs(30),
    }
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

    #[test]
    fn attempt_delay_stimmt_mit_backoff_schedule_ueberein() {
        let schedule: Vec<Duration> = backoff_schedule().take(6).collect();
        for (i, expected) in schedule.iter().enumerate() {
            let attempt = (i + 1) as u32;
            assert_eq!(attempt_delay(attempt), *expected, "attempt {attempt}");
        }
    }

    #[test]
    fn attempt_delay_null_verhaelt_sich_wie_eins() {
        assert_eq!(attempt_delay(0), attempt_delay(1));
    }

    #[test]
    fn attempt_delay_bleibt_dauerhaft_bei_30s() {
        assert_eq!(attempt_delay(10), Duration::from_secs(30));
        assert_eq!(attempt_delay(1000), Duration::from_secs(30));
    }
}
