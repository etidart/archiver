use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::sync::{Arc, Condvar, Mutex};

struct Inner {
    buffer: VecDeque<u8>,
    capacity: usize,
    writer_alive: bool,
}

pub struct Writer {
    inner: Arc<(Mutex<Inner>, Condvar, Condvar)>,
}

pub struct Reader {
    inner: Arc<(Mutex<Inner>, Condvar, Condvar)>,
}

pub fn byte_channel(capacity: usize) -> (Writer, Reader) {
    assert!(capacity > 0);
    let inner = Arc::new((
        Mutex::new(Inner {
            buffer: VecDeque::with_capacity(capacity),
            capacity,
            writer_alive: true,
        }),
        Condvar::new(),
        Condvar::new(),
    ));
    let writer = Writer {
        inner: Arc::clone(&inner),
    };
    let reader = Reader { inner };
    (writer, reader)
}

impl Write for Writer {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let (lock, not_full, not_empty) = &*self.inner;
        let mut inner = lock.lock().unwrap();

        while inner.buffer.len() == inner.capacity && inner.writer_alive {
            inner = not_full.wait(inner).unwrap();
        }

        if !inner.writer_alive {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "reader dropped"));
        }

        let available = inner.capacity - inner.buffer.len();
        let n = buf.len().min(available);

        inner.buffer.extend(&buf[..n]);

        not_empty.notify_one();

        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Read for Reader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let (lock, not_full, not_empty) = &*self.inner;
        let mut inner = lock.lock().unwrap();

        while inner.buffer.is_empty() && inner.writer_alive {
            inner = not_empty.wait(inner).unwrap();
        }

        if inner.buffer.is_empty() {
            return Ok(0);
        }

        let n = buf.len().min(inner.buffer.len());
        for dst in buf[..n].iter_mut() {
            *dst = inner.buffer.pop_front().unwrap();
        }

        not_full.notify_one();

        Ok(n)
    }
}

impl Drop for Writer {
    fn drop(&mut self) {
        let (lock, _not_full, not_empty) = &*self.inner;
        if let Ok(mut inner) = lock.lock() {
            inner.writer_alive = false;
            not_empty.notify_all();
        }
    }
}

unsafe impl Send for Writer {}
unsafe impl Send for Reader {}
unsafe impl Sync for Writer {}
unsafe impl Sync for Reader {}
