#[allow(dead_code)]
pub struct SpreadCalculator {
    binance_fee: f64,
    mexc_fee: f64,
}

/// Spread data
#[derive(Debug, Clone)]
pub struct Spread {
    pub binance_price: f64,
    pub mexc_price: f64,
    pub spread_percent: f64,
    pub net_spread_percent: f64,
}

impl SpreadCalculator {
    pub fn new(binance_fee: f64, mexc_fee: f64) -> Self {
        Self {
            binance_fee,
            mexc_fee,
        }
    }
    
    #[inline]
    pub fn calculate_raw_spread(&self, mexc_price: f64, binance_price: f64) -> f64 {
        if binance_price == 0.0 {
            return 0.0;
        }
        ((mexc_price - binance_price) / binance_price) * 100.0
    }
    
    #[inline]
    pub fn calculate_net_spread(&self, mexc_price: f64, binance_price: f64) -> f64 {
        let raw_spread = self.calculate_raw_spread(mexc_price, binance_price);
        let total_fees = (self.binance_fee + self.mexc_fee) * 100.0;
        raw_spread - total_fees
    }
    
    #[inline]
    pub fn is_opportunity(&self, spread: f64, threshold: f64) -> bool {
        spread.abs() > threshold
    }
    
    /// Calculate full spread data from prices
    pub fn calculate_spread_data(&self, binance_price: f64, mexc_price: f64) -> Spread {
        let raw_spread = self.calculate_raw_spread(mexc_price, binance_price);
        let net_spread = self.calculate_net_spread(mexc_price, binance_price);
        
        Spread {
            binance_price,
            mexc_price,
            spread_percent: raw_spread,
            net_spread_percent: net_spread,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_raw_spread_calculation() {
        let calc = SpreadCalculator::new(0.0004, 0.0002);
        let spread = calc.calculate_raw_spread(50000.0, 49000.0);
        assert!((spread - 2.04).abs() < 0.01);
    }
    
    #[test]
    fn test_net_spread_with_fees() {
        let calc = SpreadCalculator::new(0.0004, 0.0002);
        let net_spread = calc.calculate_net_spread(50000.0, 49000.0);
        // Raw spread ~2.04%, fees 0.06%, net ~1.98%
        assert!((net_spread - 1.98).abs() < 0.01);
    }
}
