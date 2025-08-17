use portable_std::io::{self, Read, Write};
use core::net::{IpAddr, SocketAddr};

#[derive(Debug)]
pub struct TcpStream;

impl TcpStream {
    pub fn connect(_address: impl AsRef<str>) -> io::Result<Self> {
        todo!()
    }
    
    fn connect_addr(addr: SocketAddr) -> io::Result<Self> {
        todo!()
    }
}

impl Read for &TcpStream {
    fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
        todo!()
    }
}

impl Read for TcpStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        (&*self).read(buf)
    }
}

impl Write for &TcpStream {
    fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
        todo!()
    }
    
    fn flush(&mut self) -> io::Result<()> {
        todo!()
    }
}

impl Write for TcpStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        (&*self).write(buf)
    }
    
    fn flush(&mut self) -> io::Result<()> {
        (&*self).flush()
    }
}

impl embedded_io::ErrorType for TcpStream {
    type Error = io::Error;
}

impl embedded_io_async::Read for TcpStream {
    async fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        todo!()
    }
}

impl embedded_io_async::Write for TcpStream {
    async fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        todo!()
    }
    
    async fn flush(&mut self) -> io::Result<()> {
        todo!()
    }
}

pub struct TcpStack;

impl embedded_nal_async::TcpConnect for TcpStack {
    type Error = io::Error;
    type Connection<'a> = TcpStream;
    
    async fn connect<'a>(
        &'a self,
        remote: SocketAddr,
    ) -> Result<Self::Connection<'a>, Self::Error> {
        todo!()
    }
}

pub struct DnsResolver;

impl embedded_nal_async::Dns for DnsResolver {
    type Error = io::Error;
    
    async fn get_host_by_name(
        &self,
        host: &str,
        addr_type: embedded_nal_async::AddrType,
    ) -> Result<IpAddr, Self::Error>
    {
        // TODO:
        Err(io::Error::other("todo"))
    }
    
    async fn get_host_by_address(
        &self,
        addr: IpAddr,
        result: &mut [u8],
    ) -> Result<usize, Self::Error>
    {
        Err(io::Error::other("unimplemented"))
    }
}
