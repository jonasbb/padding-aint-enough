use serde_json::ser::Formatter;
use std::io;

#[derive(Debug, Default)]
pub struct JsonlFormatter {
    nesting_level: usize,
}

impl JsonlFormatter {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Formatter for JsonlFormatter {
    #[inline]
    fn begin_object<W: ?Sized>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: io::Write,
    {
        self.nesting_level += 1;
        writer.write_all(b"{")
    }

    #[inline]
    fn end_object<W: ?Sized>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: io::Write,
    {
        self.nesting_level -= 1;
        writer.write_all(b"}")?;

        if self.nesting_level == 0 {
            writer.write_all(b"\n")?;
        }
        Ok(())
    }
}
