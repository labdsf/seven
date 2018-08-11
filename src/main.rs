extern crate byteorder;
extern crate crc;

use byteorder::{LittleEndian, WriteBytesExt};
use crc::crc32;
use std::env;
use std::fs::File;
use std::io;
use std::io::{Seek, SeekFrom};
use std::io::BufWriter;
use std::io::{Read, Write};

fn write_varnum<T: Write>(w: &mut T, value: u64) -> io::Result<()> {
    let mut mask: u8 = 0x80;
    let mut first_byte: u8 = 0;
    let mut i = 0;
    let mut v = value;
    while i < 8 {
        if v < 1u64 << 7 * (i + 1) {
            first_byte |= (v >> 8 * i) as u8;
            break;
        }
        first_byte = first_byte | mask;
        mask >>= 1;
        i += 1;
    }
    w.write_u8(first_byte)?;
    while i > 0 {
        w.write_u8(v as u8)?;
        v >>= 8;
        i -= 1;
    }
    Ok(())
}

fn write_header<T: Write + Seek>(filename: &str, w: &mut T, payload_len: u64) -> io::Result<()> {
    w.write_u8(1)?; // kHeader
    w.write_u8(4)?; // kMainStreamsInfo

    w.write_u8(6)?; // kPackInfo
    write_varnum(w, 0)?; // Data offset
    write_varnum(w, 1)?; // packSizes

    w.write_u8(9)?; // kSize
    write_varnum(w, payload_len)?;
    w.write_u8(0)?; // kEnd

    // Coders info
    w.write_u8(7)?; // kUnpackInfo

    w.write_u8(0x0b)?; // kFolder
    write_varnum(w, 0x01)?; // NumFolders
    w.write_u8(0x00)?; // Always 0

    // WriteFolder
    write_varnum(w, 0x01)?; // Number of coders
    w.write_u8(0x01)?; // ID size
    w.write_u8(0x00)?; // Method ID (copy)

    w.write_u8(0x0c)?; // kCodersUnPackSize
    write_varnum(w, payload_len)?; // VLQ unPackSize
    w.write_u8(0x00)?; // kEnd

    w.write_u8(0x00)?; // kEnd (UnpackInfo)

    w.write_u8(0x05)?; // kFilesInfo
    w.write_u8(0x01)?; // Number of files
    w.write_u8(0x11)?; // kName
    write_varnum(w, 1 + filename.len() as u64 * 2 + 2)?; // Property size
    w.write_u8(0x00)?; // Do not use external stream

    // Filename
    for c in filename.chars() {
        w.write_u16::<LittleEndian>(c as u16)?;
    }
    w.write_u8(0)?;
    w.write_u8(0)?;

    w.write_u8(0x00)?; // End of properties
    w.write_u8(0x00)?; // kEnd
    Ok(())
}

fn write_archive<R: Read, W: Write + Seek>(filename: &str, r: &mut R, w: &mut W) -> io::Result<()> {
    let signature: &[u8] = &[b'7', b'z', 0xBC, 0xAF, 0x27, 0x1C];
    w.write(signature)?;
    w.write(&[0u8, 4u8])?; // version
    w.seek(SeekFrom::Current(24))?; // Reserve space for start header.

    let payload_len = io::copy(r, w)?;

    use std::io::Cursor;
    let mut header_buffer: Cursor<Vec<u8>> = Cursor::new(Vec::new());
    write_header(filename, &mut header_buffer, payload_len)?;
    let header_size = header_buffer.get_ref().len();
    let header_crc32: u32 = crc32::checksum_ieee(header_buffer.into_inner().as_slice());

    write_header(filename, w, payload_len)?;

    // Start header
    w.seek(SeekFrom::Start(6 + 2))?;
    let mut start_header_buffer: Cursor<Vec<u8>> = Cursor::new(Vec::new());
    start_header_buffer
        .write_u64::<LittleEndian>(payload_len)?; // Next header offset
    start_header_buffer
        .write_u64::<LittleEndian>(header_size as u64)?; // Next header size
    start_header_buffer.write_u32::<LittleEndian>(header_crc32)?; // Next header CRC
    let start_header_crc32: u32 = crc32::checksum_ieee(start_header_buffer.get_ref().as_slice());

    w.write_u32::<LittleEndian>(start_header_crc32)?; // Start header CRC
    w.write(start_header_buffer.into_inner().as_slice())?;
    Ok(())
}

fn main() {
    for filename in env::args().skip(1) {
        let mut reader = File::open(&filename).unwrap();

        let file = File::create(filename.clone() + ".7z").unwrap();
        let mut writer = BufWriter::new(file);

        write_archive(filename.as_str(), &mut reader, &mut writer).unwrap();
    }
}
