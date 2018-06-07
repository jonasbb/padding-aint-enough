use byteorder::*;
use constants::*;
use std::io::Cursor;
use std::io::{self, Read};

#[derive(Clone, Debug)]
pub struct DecoderReader<R: Read> {
    reader: R,
    content_type: Option<String>,
    saw_start: bool,
}

#[derive(Debug)]
pub enum DecodeError {
    Io(io::Error),
    InvalidMagicBytes { magic_bytes: u32 },
    UnknownFieldsInHeader { magic_bytes: u32 },
    UnwantedContentType,
    TooShortFrameLength,
    DuplicateStartFrame,
    InvalidLength,
}

impl From<io::Error> for DecodeError {
    fn from(error: io::Error) -> DecodeError {
        DecodeError::Io(error)
    }
}

pub enum Frame {
    Content(Vec<u8>),
    Start,
    Stop,
}

impl<R: Read> DecoderReader<R> {
    pub fn new(reader: R, content_type: Option<String>) -> DecoderReader<R> {
        DecoderReader {
            reader,
            content_type,
            saw_start: false,
        }
    }

    pub fn read_frame(&mut self) -> Result<Frame, DecodeError> {
        match self.reader.read_u32::<BigEndian>()? {
            CONTROL_ESCAPE => self.read_escape_frame(),
            length => self.read_content_frame(length as usize),
        }
    }

    fn read_escape_frame(&mut self) -> Result<Frame, DecodeError> {
        trace!("Escape Frame");
        let frame_length = self.reader.read_u32::<BigEndian>()? as usize;
        if frame_length < 4 {
            return Err(DecodeError::TooShortFrameLength);
        }
        trace!("Frame Length: {}", frame_length);
        match self.reader.read_u32::<BigEndian>()? {
            CONTROL_START => self.read_start_frame(frame_length),
            CONTROL_STOP => self.read_stop_frame(frame_length),
            unkwn => Err(DecodeError::InvalidMagicBytes { magic_bytes: unkwn }),
        }
    }

    fn read_start_frame(&mut self, mut frame_length: usize) -> Result<Frame, DecodeError> {
        // substract size of length field
        frame_length -= 4;
        let mut buffer = vec![0; frame_length];
        self.reader.read_exact(&mut *buffer)?;
        trace!("Frame {:?}", buffer);
        let mut frame = Cursor::new(buffer);
        while frame.position() != frame_length as u64 {
            match frame.read_u32::<BigEndian>()? {
                CONTROL_FIELD_CONTENT_TYPE => {
                    trace!("Has content type");
                    let content_type_length = frame.read_u32::<BigEndian>()? as usize;
                    trace!("Content Type Length: {}", content_type_length);
                    let mut content_type = vec![0; content_type_length];
                    frame.read_exact(&mut *content_type)?;
                    trace!("Content Type {:?}", content_type);
                    if let Some(ref expected_content_type) = self.content_type {
                        if expected_content_type.as_bytes() != &*content_type {
                            return Err(DecodeError::UnwantedContentType);
                        }
                    }
                }
                magic_bytes => return Err(DecodeError::UnknownFieldsInHeader { magic_bytes }),
            }
        }

        Ok(Frame::Start)
    }

    fn read_stop_frame(&mut self, frame_length: usize) -> Result<Frame, DecodeError> {
        if frame_length != 4 {
            Err(DecodeError::InvalidLength)
        } else {
            Ok(Frame::Stop)
        }
    }

    fn read_content_frame(&mut self, frame_length: usize) -> Result<Frame, DecodeError> {
        trace!("Content Frame Length: {}", frame_length);
        let mut buffer = vec![0; frame_length];
        self.reader.read_exact(&mut *buffer)?;
        Ok(Frame::Content(buffer))
    }
}

impl<R: Read> Iterator for DecoderReader<R> {
    type Item = Result<Vec<u8>, DecodeError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.read_frame() {
            Ok(Frame::Start) => {
                if self.saw_start {
                    Some(Err(DecodeError::DuplicateStartFrame))
                } else {
                    self.saw_start = true;
                    self.next()
                }
            }
            Ok(Frame::Stop) => None,
            Ok(Frame::Content(content)) => Some(Ok(content)),
            Err(err) => Some(Err(err)),
        }
    }
}

#[test]
fn test_fstrm() {
    let data = include_bytes!("../test.fstrm");
    let rdr = DecoderReader::new(Cursor::new(&data[..]), None);

    let expected = vec![
        b"Hello, world #0\n",
        b"Hello, world #1\n",
        b"Hello, world #2\n",
        b"Hello, world #3\n",
        b"Hello, world #4\n",
        b"Hello, world #5\n",
        b"Hello, world #6\n",
        b"Hello, world #7\n",
        b"Hello, world #8\n",
        b"Hello, world #9\n",
    ];

    for (expected, read) in expected.into_iter().zip(rdr) {
        assert_eq!(&*expected, &*read.unwrap());
    }
}
