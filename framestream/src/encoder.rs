use byteorder::*;
use constants::*;
use std::io::{self, Write};
use std::mem;

#[derive(Clone, Debug)]
pub struct EncoderWriter<W: Write> {
    writer: Option<W>,
    content_type: Option<String>,
    partial: bool,
    started: bool,
}

impl<W: Write> Write for EncoderWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if !self.started {
            self.started = true;
            try!(self.write_control_start());
        }
        self.write_frame(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.as_mut().unwrap().flush()
    }
}

impl<W: Write> EncoderWriter<W> {
    pub fn new(writer: W, content_type: Option<String>) -> EncoderWriter<W> {
        EncoderWriter {
            writer: Some(writer),
            content_type,
            partial: false,
            started: false,
        }
    }

    pub fn finish(mut self) -> io::Result<W> {
        if self.started {
            try!(self.write_control_stop());
            self.started = false;
        }
        Ok(self.writer.take().unwrap())
    }

    pub fn reset(&mut self, writer: W) -> io::Result<W> {
        if self.started {
            try!(self.write_control_stop());
            self.started = false;
        }
        Ok(mem::replace(&mut self.writer, Some(writer)).unwrap())
    }

    pub fn into_inner(mut self) -> W {
        self.writer.take().unwrap()
    }

    fn write_control_start(&mut self) -> io::Result<()> {
        let mut total_len = 0; // Escape
        total_len += 4; // Frame length
        total_len += 4; // Control type length
        total_len += 4; // Control type
        if let Some(ref content_type) = self.content_type {
            total_len += 4; // CONTROL_FIELD_CONTENT_TYPE
            total_len += 4; // Length of content type string
            total_len += content_type.as_bytes().len();
        }
        let mut buf = Vec::with_capacity(total_len);
        let _ = buf.write_u32::<BigEndian>(0); // Escape
        let _ = buf.write_u32::<BigEndian>(total_len as u32 - 2 * 4); // Frame length
        let _ = buf.write_u32::<BigEndian>(CONTROL_START); // Control type
        if let Some(ref content_type) = self.content_type {
            let _ = buf.write_u32::<BigEndian>(CONTROL_FIELD_CONTENT_TYPE);
            let _ = buf.write_u32::<BigEndian>(content_type.as_bytes().len() as u32);
            let _ = buf.write(content_type.as_bytes());
        }
        try!(self.writer.as_mut().unwrap().write_all(&buf));
        Ok(())
    }

    fn write_control_stop(&mut self) -> io::Result<()> {
        let total_len = 3 * 4;
        let mut buf = Vec::with_capacity(total_len);
        let _ = buf.write_u32::<BigEndian>(0); // Escape
        let _ = buf.write_u32::<BigEndian>(total_len as u32 - 2 * 4); // Frame length
        let _ = buf.write_u32::<BigEndian>(CONTROL_STOP); // Control type
        try!(self.writer.as_mut().unwrap().write_all(&buf));
        Ok(())
    }

    fn write_frame(&mut self, frame: &[u8]) -> io::Result<usize> {
        let len = frame.len();
        if !self.partial {
            let len_u32 = [
                (len >> 24) as u8,
                (len >> 16) as u8,
                (len >> 8) as u8,
                len as u8,
            ];
            try!(self.writer.as_mut().unwrap().write_all(&len_u32));
        }
        let wlen = try!(self.writer.as_mut().unwrap().write(frame));
        self.partial = wlen < len;
        Ok(wlen)
    }
}
