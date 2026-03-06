/// Lock-free ring buffer для HFT (фиксированный размер)
/// Оптимизирован для single-writer, multiple-readers
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug, Clone, Copy)]
pub struct PriceSnapshot {
    pub price: f64,
    pub exchange_timestamp: i64,
    pub local_received: i64,
}

impl Default for PriceSnapshot {
    fn default() -> Self {
        Self {
            price: 0.0,
            exchange_timestamp: 0,
            local_received: 0,
        }
    }
}

impl PriceSnapshot {
    /// Вычисляет network delay для этой сделки
    pub fn network_delay(&self) -> i64 {
        self.local_received - self.exchange_timestamp
    }
    
    /// Вычисляет real time сделки (exchange_ts + network_delay)
    pub fn real_time(&self) -> i64 {
        self.exchange_timestamp + self.network_delay()
    }
}

#[derive(Debug)]
pub struct RingBuffer {
    buffer: Vec<PriceSnapshot>,
    capacity: usize,
    write_index: AtomicUsize,
}

impl Clone for RingBuffer {
    fn clone(&self) -> Self {
        // Для клонирования создаём новый буфер с теми же данными
        let current_idx = self.write_index.load(Ordering::Acquire);
        let mut new_buffer = vec![PriceSnapshot::default(); self.capacity];
        
        // Копируем данные
        for i in 0..self.capacity {
            unsafe {
                let src_ptr = self.buffer.as_ptr().add(i);
                let dst_ptr = new_buffer.as_mut_ptr().add(i);
                std::ptr::copy_nonoverlapping(src_ptr, dst_ptr, 1);
            }
        }
        
        Self {
            buffer: new_buffer,
            capacity: self.capacity,
            write_index: AtomicUsize::new(current_idx),
        }
    }
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
        let write_idx = idx % self.capacity;
        let next_idx = idx + 1;
        
        // SAFETY: Только один writer, поэтому безопасно
        unsafe {
            let ptr = self.buffer.as_mut_ptr().add(write_idx);
            std::ptr::write(ptr, snapshot);
        }
        
        self.write_index.store(next_idx, Ordering::Release);
    }
    
    /// Возвращает последние N элементов (для readers)
    pub fn get_last_n(&self, n: usize) -> Vec<PriceSnapshot> {
        let current_idx = self.write_index.load(Ordering::Acquire);
        if current_idx == 0 {
            return Vec::new();
        }
        
        let count = n.min(self.capacity).min(current_idx as usize);
        let mut result = Vec::with_capacity(count);
        
        for i in 0..count {
            let idx = (current_idx as usize - count + i) % self.capacity;
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
        
        let idx = ((current_idx as usize - 1) % self.capacity) as usize;
        // SAFETY: Индекс всегда в пределах capacity
        unsafe {
            let ptr = self.buffer.as_ptr().add(idx);
            Some(std::ptr::read(ptr))
        }
    }
    
    /// Возвращает количество элементов
    pub fn len(&self) -> usize {
        let idx = self.write_index.load(Ordering::Acquire);
        if idx == 0 {
            0
        } else {
            self.capacity.min(idx as usize)
        }
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
        
        rb.push(PriceSnapshot { price: 100.0, exchange_timestamp: 1, local_received: 1000 });
        rb.push(PriceSnapshot { price: 101.0, exchange_timestamp: 2, local_received: 1001 });
        rb.push(PriceSnapshot { price: 102.0, exchange_timestamp: 3, local_received: 1002 });
        
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.last().unwrap().price, 102.0);
        
        // Overflow - должен перезаписать первый элемент
        rb.push(PriceSnapshot { price: 103.0, exchange_timestamp: 4, local_received: 1003 });
        assert_eq!(rb.len(), 3);
        
        let last_n = rb.get_last_n(3);
        assert_eq!(last_n.len(), 3);
        assert_eq!(last_n[2].price, 103.0);
    }
}
