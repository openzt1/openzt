use std::collections::HashSet;
use std::ops::Range;

#[derive(Debug, Clone)]
pub struct PortPool {
    rdp_range: Range<u16>,
    console_range: Range<u16>,
    xpra_range: Range<u16>,
    allocated_rdp: HashSet<u16>,
    allocated_console: HashSet<u16>,
    allocated_xpra: HashSet<u16>,
}

impl PortPool {
    pub fn new(rdp_range: Range<u16>, console_range: Range<u16>, xpra_range: Range<u16>) -> Self {
        Self {
            rdp_range,
            console_range,
            xpra_range,
            allocated_rdp: HashSet::new(),
            allocated_console: HashSet::new(),
            allocated_xpra: HashSet::new(),
        }
    }

    pub fn allocate_rdp(&mut self) -> Option<u16> {
        for port in self.rdp_range.clone() {
            if !self.allocated_rdp.contains(&port) {
                self.allocated_rdp.insert(port);
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

    pub fn allocate_pair(&mut self) -> Option<(u16, u16)> {
        let rdp_port = self.allocate_rdp()?;
        let console_port = self.allocate_console()?;
        Some((rdp_port, console_port))
    }

    pub fn allocate_xpra(&mut self) -> Option<u16> {
        for port in self.xpra_range.clone() {
            if !self.allocated_xpra.contains(&port) {
                self.allocated_xpra.insert(port);
                return Some(port);
            }
        }
        None
    }

    /// Allocate all three ports (RDP, Console, XPRA) as a triplet
    pub fn allocate_triplet(&mut self) -> Option<(u16, u16, u16)> {
        let rdp_port = self.allocate_rdp()?;
        let console_port = self.allocate_console()?;
        let xpra_port = self.allocate_xpra()?;
        Some((rdp_port, console_port, xpra_port))
    }

    pub fn release_rdp(&mut self, port: u16) {
        self.allocated_rdp.remove(&port);
    }

    pub fn release_console(&mut self, port: u16) {
        self.allocated_console.remove(&port);
    }

    pub fn release_pair(&mut self, rdp_port: u16, console_port: u16) {
        self.release_rdp(rdp_port);
        self.release_console(console_port);
    }

    pub fn release_xpra(&mut self, port: u16) {
        self.allocated_xpra.remove(&port);
    }

    /// Release all three ports as a triplet
    pub fn release_triplet(&mut self, rdp_port: u16, console_port: u16, xpra_port: u16) {
        self.release_rdp(rdp_port);
        self.release_console(console_port);
        self.release_xpra(xpra_port);
    }

    pub fn rdp_available(&self) -> usize {
        self.rdp_range.clone().count() - self.allocated_rdp.len()
    }

    pub fn console_available(&self) -> usize {
        self.console_range.clone().count() - self.allocated_console.len()
    }

    pub fn xpra_available(&self) -> usize {
        self.xpra_range.clone().count() - self.allocated_xpra.len()
    }

    /// Add an existing RDP port allocation (for recovery)
    pub fn add_existing_rdp(&mut self, port: u16) -> anyhow::Result<()> {
        if !self.rdp_range.contains(&port) {
            return Err(anyhow::anyhow!("Port {} outside RDP range {:?}", port, self.rdp_range));
        }
        self.allocated_rdp.insert(port);
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

    /// Add an existing XPRA port allocation (for recovery)
    pub fn add_existing_xpra(&mut self, port: u16) -> anyhow::Result<()> {
        if !self.xpra_range.contains(&port) {
            return Err(anyhow::anyhow!("Port {} outside XPRA range {:?}", port, self.xpra_range));
        }
        self.allocated_xpra.insert(port);
        Ok(())
    }

    /// Add an existing port pair allocation (for recovery)
    pub fn add_existing_pair(&mut self, rdp_port: u16, console_port: u16) -> anyhow::Result<()> {
        self.add_existing_rdp(rdp_port)?;
        self.add_existing_console(console_port)?;
        Ok(())
    }

    /// Add an existing port triplet allocation (for recovery)
    pub fn add_existing_triplet(&mut self, rdp_port: u16, console_port: u16, xpra_port: u16) -> anyhow::Result<()> {
        self.add_existing_rdp(rdp_port)?;
        self.add_existing_console(console_port)?;
        self.add_existing_xpra(xpra_port)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allocate_pair() {
        let mut pool = PortPool::new(3390..3395, 8081..8086, 14500..14505);
        let (rdp, console) = pool.allocate_pair().unwrap();
        assert_eq!(rdp, 3390);
        assert_eq!(console, 8081);
    }

    #[test]
    fn test_exhaustion() {
        let mut pool = PortPool::new(3390..3392, 8081..8083, 14500..14502);
        pool.allocate_triplet().unwrap();
        pool.allocate_triplet().unwrap();
        assert!(pool.allocate_triplet().is_none());
    }

    #[test]
    fn test_release() {
        let mut pool = PortPool::new(3390..3392, 8081..8083, 14500..14502);
        let (rdp, console, xpra) = pool.allocate_triplet().unwrap();
        pool.release_triplet(rdp, console, xpra);
        assert_eq!(pool.allocate_triplet().unwrap(), (rdp, console, xpra));
    }

    #[test]
    fn test_allocate_triplet() {
        let mut pool = PortPool::new(3390..3395, 8081..8086, 14500..14505);
        let (rdp, console, xpra) = pool.allocate_triplet().unwrap();
        assert_eq!(rdp, 3390);
        assert_eq!(console, 8081);
        assert_eq!(xpra, 14500);
    }
}
