#[allow(dead_code)]
pub struct SymbolNormalizer;

impl SymbolNormalizer {
    /// Normalize Binance format: BTCUSDT -> BTC/USDT
    #[inline]
    pub fn normalize_binance(symbol: &str) -> String {
        if symbol.contains("USDT") {
            symbol.replace("USDT", "/USDT")
        } else {
            symbol.to_string()
        }
    }
    
    /// Normalize MEXC format: BTC_USDT -> BTC/USDT
    #[inline]
    pub fn normalize_mexc(symbol: &str) -> String {
        symbol.replace('_', "/")
    }
    
    /// Convert any format to canonical: BTC/USDT
    #[inline]
    pub fn to_canonical(symbol: &str) -> String {
        symbol.to_uppercase().replace('_', "/")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_normalize_binance() {
        assert_eq!(SymbolNormalizer::normalize_binance("BTCUSDT"), "BTC/USDT");
    }
    
    #[test]
    fn test_normalize_mexc() {
        assert_eq!(SymbolNormalizer::normalize_mexc("BTC_USDT"), "BTC/USDT");
    }
    
    #[test]
    fn test_to_canonical() {
        assert_eq!(SymbolNormalizer::to_canonical("btc_usdt"), "BTC/USDT");
        assert_eq!(SymbolNormalizer::to_canonical("BTCUSDT"), "BTCUSDT");
    }
}
