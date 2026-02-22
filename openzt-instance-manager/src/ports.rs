use std::collections::HashSet;
use std::ops::Range;

#[derive(Debug, Clone)]
pub struct PortPool {
    vnc_range: Range<u16>,
    console_range: Range<u16>,
    allocated_vnc: HashSet<u16>,
    allocated_console: HashSet<u16>,
}

impl PortPool {
    pub fn new(vnc_range: Range<u16>, console_range: Range<u16>) -> Self {
        Self {
            vnc_range,
            console_range,
            allocated_vnc: HashSet::new(),
            allocated_console: HashSet::new(),
        }
    }

    pub fn allocate_vnc(&mut self) -> Option<u16> {
        for port in self.vnc_range.clone() {
            if !self.allocated_vnc.contains(&port) {
                self.allocated_vnc.insert(port);
                return Some(port);
            }
        }
        None
    }

    pub fn allocate_console(&mut self) -> Option<u16> {
        for port in self.console_range.clone() {
            if !self.allocated_console.contains(&port) {
                self.allocated_console.insert(port);
                return Some(port);
            }
        }
        None
    }

    /// Allocate both ports (VNC, Console) as a pair
    pub fn allocate_pair(&mut self) -> Option<(u16, u16)> {
        let vnc_port = self.allocate_vnc()?;
        let console_port = self.allocate_console()?;
        Some((vnc_port, console_port))
    }

    pub fn release_vnc(&mut self, port: u16) {
        self.allocated_vnc.remove(&port);
    }

    pub fn release_console(&mut self, port: u16) {
        self.allocated_console.remove(&port);
    }

    /// Release both ports as a pair
    pub fn release_pair(&mut self, vnc_port: u16, console_port: u16) {
        self.release_vnc(vnc_port);
        self.release_console(console_port);
    }

    pub fn vnc_available(&self) -> usize {
        self.vnc_range.clone().count() - self.allocated_vnc.len()
    }

    pub fn console_available(&self) -> usize {
        self.console_range.clone().count() - self.allocated_console.len()
    }

    /// Add an existing VNC port allocation (for recovery)
    pub fn add_existing_vnc(&mut self, port: u16) -> anyhow::Result<()> {
        if !self.vnc_range.contains(&port) {
            return Err(anyhow::anyhow!("Port {} outside VNC range {:?}", port, self.vnc_range));
        }
        self.allocated_vnc.insert(port);
        Ok(())
    }

    /// Add an existing console port allocation (for recovery)
    pub fn add_existing_console(&mut self, port: u16) -> anyhow::Result<()> {
        if !self.console_range.contains(&port) {
            return Err(anyhow::anyhow!("Port {} outside console range {:?}", port, self.console_range));
        }
        self.allocated_console.insert(port);
        Ok(())
    }

    /// Add an existing port pair allocation (for recovery)
    pub fn add_existing_pair(&mut self, vnc_port: u16, console_port: u16) -> anyhow::Result<()> {
        self.add_existing_vnc(vnc_port)?;
        self.add_existing_console(console_port)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allocate_pair() {
        let mut pool = PortPool::new(5900..5905, 8081..8086);
        let (vnc, console) = pool.allocate_pair().unwrap();
        assert_eq!(vnc, 5900);
        assert_eq!(console, 8081);
    }

    #[test]
    fn test_exhaustion() {
        let mut pool = PortPool::new(5900..5902, 8081..8083);
        pool.allocate_pair().unwrap();
        pool.allocate_pair().unwrap();
        assert!(pool.allocate_pair().is_none());
    }

    #[test]
    fn test_release() {
        let mut pool = PortPool::new(5900..5902, 8081..8083);
        let (vnc, console) = pool.allocate_pair().unwrap();
        pool.release_pair(vnc, console);
        assert_eq!(pool.allocate_pair().unwrap(), (vnc, console));
    }
}
