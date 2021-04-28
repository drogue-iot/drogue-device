use crate::parser;
use crate::protocol::Response;
use core::str::from_utf8;
use moveslice::Moveslice;

pub(crate) struct Buffer {
    buffer: [u8; 1024],
    pos: usize,
    needs_parse: bool,
}

impl Buffer {
    pub fn new() -> Self {
        Buffer {
            buffer: [0; 1024],
            pos: 0,
            needs_parse: false,
        }
    }

    pub fn write(&mut self, octet: u8) -> Result<(), u8> {
        if self.pos >= self.buffer.len() {
            Err(octet)
        } else {
            self.buffer[self.pos] = octet;
            self.pos += 1;
            self.needs_parse = true;
            Ok(())
        }
    }

    pub fn parse(&mut self) -> Result<Response, ()> {
        if self.pos == 0 {
            return Ok(Response::None);
        }
        if !self.needs_parse {
            return Ok(Response::None);
        }
        self.needs_parse = false;

        let str = from_utf8(&self.buffer[0..self.pos]);
        match str {
            Ok(_) => {
                // log::info!("parsing {} [{}]", self.pos, s);
            }
            Err(e) => {
                let _ = from_utf8(&self.buffer[0..e.valid_up_to()]).unwrap();
                // log::info!("parsing {} [{}<truncated>] ({})", self.pos, s, e);
            }
        }

        let mut ret = Ok(Response::None);

        if let Ok((remainder, response)) = parser::parse(&self.buffer[0..self.pos]) {
            let len = remainder.len();
            if len > 0 {
                let start = self.pos - len;
                (&mut self.buffer[..]).moveslice(start..start + len, 0);
                self.pos = len;
                self.needs_parse = true;
            } else {
                self.pos = 0;
            }
            ret = Ok(response);
        }

        /*
        let mut dump_len = self.pos;
        if dump_len > 10 {
            dump_len = 10;
        }

        log::info!("-------->>");
        log::info!("remainder: {} bytes", self.pos);
        for (i, b) in self.buffer[0..dump_len].iter().enumerate() {
            log::info!( "{}: {} {:#x?}", i, *b as char, *b );
        }

        let remainder = from_utf8(&self.buffer[0..self.pos]);

        match remainder {
            Ok(s) => {
                log::info!("{}", s);
            },
            Err(e) => {
                let s = from_utf8( &self.buffer[0..e.valid_up_to()]).unwrap();
                log::info!("{}<truncated>", s)
            },
        }

        let mut dump_len = self.pos;
        if dump_len > 30 {
            dump_len = 30;
        }

        log::info!("-------->>");
        for (i, b ) in self.buffer[self.pos - dump_len..self.pos].iter().enumerate() {
            log::info!( "{}: {} {:#x?}", i + (self.pos - dump_len), *b as char, *b );
        }
        log::info!("<<--------");


         */
        //Ok(Response::None)
        ret
    }
}
