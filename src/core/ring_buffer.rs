/// Lock-free ring buffer для HFT (фиксированный размер)
/// Оптимизирован для single-writer, multiple-readers
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug, Clone, Copy)]
pub struct PriceSnapshot {
    pub price: f64,
    pub timestamp: i64,
}

impl Default for PriceSnapshot {
    fn default() -> Self {
        Self {
            price: 0.0,
            timestamp: 0,
        }
    }
}

pub struct RingBuffer {
    buffer: Vec<PriceSnapshot>,
    capacity: usize,
    write_index: AtomicUsize,
}

impl RingBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: vec![PriceSnapshot::default(); capacity],
            capacity,
            write_index: AtomicUsize::new(0),
        }
    }
    
    /// Добавляет элемент (только один writer!)
    pub fn push(&mut self, snapshot: PriceSnapshot) {
        let idx = self.write_index.load(Ordering::Relaxed);
        let next_idx = (idx + 1) % self.capacity;
        
        // SAFETY: Только один writer, поэтому безопасно
        unsafe {
            let ptr = self.buffer.as_mut_ptr().add(idx);
            std::ptr::write(ptr, snapshot);
        }
        
        self.write_index.store(next_idx, Ordering::Release);
    }
    
    /// Возвращает последние N элементов (для readers)
    pub fn get_last_n(&self, n: usize) -> Vec<PriceSnapshot> {
        let current_idx = self.write_index.load(Ordering::Acquire);
        let count = n.min(self.capacity);
        let mut result = Vec::with_capacity(count);
        
        for i in 0..count {
            let idx = (current_idx + self.capacity - count + i) % self.capacity;
            // SAFETY: Индекс всегда в пределах capacity
            unsafe {
                let ptr = self.buffer.as_ptr().add(idx);
                result.push(std::ptr::read(ptr));
            }
        }
        
        result
    }
    
    /// Возвращает последний элемент
    pub fn last(&self) -> Option<PriceSnapshot> {
        let current_idx = self.write_index.load(Ordering::Acquire);
        if current_idx == 0 {
            return None;
        }
        
        let idx = (current_idx + self.capacity - 1) % self.capacity;
        // SAFETY: Индекс всегда в пределах capacity
        unsafe {
            let ptr = self.buffer.as_ptr().add(idx);
            Some(std::ptr::read(ptr))
        }
    }
    
    /// Возвращает количество элементов
    pub fn len(&self) -> usize {
        self.write_index.load(Ordering::Acquire).min(self.capacity)
    }
    
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

// SAFETY: RingBuffer безопасен для Send/Sync при single-writer
unsafe impl Send for RingBuffer {}
unsafe impl Sync for RingBuffer {}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_ring_buffer() {
        let mut rb = RingBuffer::new(3);
        
        rb.push(PriceSnapshot { price: 100.0, timestamp: 1 });
        rb.push(PriceSnapshot { price: 101.0, timestamp: 2 });
        rb.push(PriceSnapshot { price: 102.0, timestamp: 3 });
        
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.last().unwrap().price, 102.0);
        
        // Overflow - должен перезаписать первый элемент
        rb.push(PriceSnapshot { price: 103.0, timestamp: 4 });
        assert_eq!(rb.len(), 3);
        
        let last_n = rb.get_last_n(3);
        assert_eq!(last_n.len(), 3);
        assert_eq!(last_n[2].price, 103.0);
    }
}
