//! Kleine, pure Helfer, die reine Entscheidungslogik von I/O trennen — testbar ohne
//! Netzwerk/Zeit-Mocking, weil sie nur mit Werten statt echten Uhren arbeiten.

/// Byte-Schwelle für [`should_flush`]: 32 KiB.
pub const FLUSH_BYTES_THRESHOLD: usize = 32 * 1024;

/// Zeit-Schwelle (ms seit dem ersten ungeflushten Byte) für [`should_flush`].
pub const FLUSH_MS_THRESHOLD: u64 = 10;

/// Batching-Entscheidung für den PTY-Output-Forwarder (Task 3): true, sobald entweder
/// `buffered_bytes` die 32-KiB-Schwelle erreicht/überschreitet ODER seit dem ERSTEN
/// ungeflushten Byte mindestens 10 ms vergangen sind. `elapsed_ms_since_first` ist `None`,
/// solange noch kein Byte im Puffer steht (dann greift nur das Byte-Kriterium — das bei
/// leerem Puffer ohnehin nie erfüllt ist).
pub fn should_flush(buffered_bytes: usize, elapsed_ms_since_first: Option<u64>) -> bool {
    buffered_bytes >= FLUSH_BYTES_THRESHOLD
        || elapsed_ms_since_first.is_some_and(|ms| ms >= FLUSH_MS_THRESHOLD)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leerer_puffer_ohne_erstes_byte_flusht_nicht() {
        assert!(!should_flush(0, None));
    }

    #[test]
    fn unter_beiden_schwellen_flusht_nicht() {
        assert!(!should_flush(100, Some(0)));
        assert!(!should_flush(
            FLUSH_BYTES_THRESHOLD - 1,
            Some(FLUSH_MS_THRESHOLD - 1)
        ));
    }

    #[test]
    fn byte_schwelle_erreicht_flusht() {
        assert!(should_flush(FLUSH_BYTES_THRESHOLD, Some(0)));
    }

    #[test]
    fn byte_schwelle_ueberschritten_flusht() {
        assert!(should_flush(FLUSH_BYTES_THRESHOLD + 1, None));
    }

    #[test]
    fn zeit_schwelle_erreicht_flusht_auch_bei_wenig_bytes() {
        assert!(should_flush(1, Some(FLUSH_MS_THRESHOLD)));
    }

    #[test]
    fn zeit_schwelle_ueberschritten_flusht() {
        assert!(should_flush(1, Some(FLUSH_MS_THRESHOLD + 5)));
    }

    #[test]
    fn knapp_unter_zeit_schwelle_flusht_nicht() {
        assert!(!should_flush(1, Some(FLUSH_MS_THRESHOLD - 1)));
    }
}
