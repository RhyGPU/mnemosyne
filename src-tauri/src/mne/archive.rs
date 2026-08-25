use std::{
    collections::HashMap,
    fs,
    io::{Cursor, Read, Seek, SeekFrom, Write},
    path::Path,
};

use super::contracts::MneBundleManifest;

pub(crate) fn validate_mne_manifest(manifest: &MneBundleManifest) -> Result<(), String> {
    if manifest.mne_version != 1 {
        return Err(format!("Unsupported .mne version {}", manifest.mne_version));
    }
    if !matches!(
        manifest.bundle_type.as_str(),
        "character_soul" | "world_setting" | "scenario_bundle" | "session_checkpoint"
    ) {
        return Err("Unsupported .mne bundle_type".into());
    }
    for path in manifest
        .contents
        .souls
        .iter()
        .chain(manifest.contents.worlds.iter())
        .chain(manifest.contents.images.iter())
    {
        validate_bundle_path(path)?;
    }
    if let Some(path) = manifest.contents.conversation.as_ref() {
        validate_bundle_path(path)?;
    }
    Ok(())
}

pub(crate) fn validate_bundle_path(path: &str) -> Result<(), String> {
    let normalized = path.replace('\\', "/");
    if normalized.trim().is_empty()
        || normalized.starts_with('/')
        || normalized.contains("../")
        || normalized == ".."
        || normalized.contains('\0')
        || Path::new(&normalized).is_absolute()
    {
        return Err("Invalid bundle path".into());
    }
    Ok(())
}

pub(crate) fn write_stored_zip(path: &Path, files: &[(String, Vec<u8>)]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let mut file = fs::File::create(path).map_err(|err| err.to_string())?;
    let mut central = Vec::new();
    for (name, data) in files {
        validate_bundle_path(name)?;
        let offset = file.stream_position().map_err(|err| err.to_string())? as u32;
        let crc = crc32(data);
        let name_bytes = name.as_bytes();
        write_u32(&mut file, 0x0403_4b50)?;
        write_u16(&mut file, 20)?;
        write_u16(&mut file, 0)?;
        write_u16(&mut file, 0)?;
        write_u16(&mut file, 0)?;
        write_u16(&mut file, 0)?;
        write_u32(&mut file, crc)?;
        write_u32(&mut file, data.len() as u32)?;
        write_u32(&mut file, data.len() as u32)?;
        write_u16(&mut file, name_bytes.len() as u16)?;
        write_u16(&mut file, 0)?;
        file.write_all(name_bytes).map_err(|err| err.to_string())?;
        file.write_all(data).map_err(|err| err.to_string())?;
        central.push((name.clone(), crc, data.len() as u32, offset));
    }
    let central_offset = file.stream_position().map_err(|err| err.to_string())? as u32;
    for (name, crc, size, offset) in &central {
        let name_bytes = name.as_bytes();
        write_u32(&mut file, 0x0201_4b50)?;
        write_u16(&mut file, 20)?;
        write_u16(&mut file, 20)?;
        write_u16(&mut file, 0)?;
        write_u16(&mut file, 0)?;
        write_u16(&mut file, 0)?;
        write_u16(&mut file, 0)?;
        write_u32(&mut file, *crc)?;
        write_u32(&mut file, *size)?;
        write_u32(&mut file, *size)?;
        write_u16(&mut file, name_bytes.len() as u16)?;
        write_u16(&mut file, 0)?;
        write_u16(&mut file, 0)?;
        write_u16(&mut file, 0)?;
        write_u16(&mut file, 0)?;
        write_u32(&mut file, 0)?;
        write_u32(&mut file, *offset)?;
        file.write_all(name_bytes).map_err(|err| err.to_string())?;
    }
    let central_size =
        file.stream_position().map_err(|err| err.to_string())? as u32 - central_offset;
    write_u32(&mut file, 0x0605_4b50)?;
    write_u16(&mut file, 0)?;
    write_u16(&mut file, 0)?;
    write_u16(&mut file, central.len() as u16)?;
    write_u16(&mut file, central.len() as u16)?;
    write_u32(&mut file, central_size)?;
    write_u32(&mut file, central_offset)?;
    write_u16(&mut file, 0)?;
    Ok(())
}

pub(crate) fn read_stored_zip(bytes: &[u8]) -> Result<HashMap<String, Vec<u8>>, String> {
    let eocd_pos = bytes
        .windows(4)
        .rposition(|window| window == [0x50, 0x4b, 0x05, 0x06])
        .ok_or_else(|| "Invalid .mne zip: missing end of central directory".to_string())?;
    let mut cursor = Cursor::new(&bytes[eocd_pos + 4..]);
    let _disk = read_u16(&mut cursor)?;
    let _central_disk = read_u16(&mut cursor)?;
    let count = read_u16(&mut cursor)? as usize;
    let _total = read_u16(&mut cursor)?;
    let _central_size = read_u32(&mut cursor)? as usize;
    let central_offset = read_u32(&mut cursor)? as usize;
    let mut cursor = Cursor::new(bytes);
    cursor
        .seek(SeekFrom::Start(central_offset as u64))
        .map_err(|err| err.to_string())?;
    let mut entries = HashMap::new();
    for _ in 0..count {
        if read_u32(&mut cursor)? != 0x0201_4b50 {
            return Err("Invalid .mne zip central directory".into());
        }
        cursor
            .seek(SeekFrom::Current(6))
            .map_err(|err| err.to_string())?;
        let compression = read_u16(&mut cursor)?;
        cursor
            .seek(SeekFrom::Current(4))
            .map_err(|err| err.to_string())?;
        let crc = read_u32(&mut cursor)?;
        let compressed_size = read_u32(&mut cursor)? as usize;
        let uncompressed_size = read_u32(&mut cursor)? as usize;
        let name_len = read_u16(&mut cursor)? as usize;
        let extra_len = read_u16(&mut cursor)? as usize;
        let comment_len = read_u16(&mut cursor)? as usize;
        cursor
            .seek(SeekFrom::Current(8))
            .map_err(|err| err.to_string())?;
        let local_offset = read_u32(&mut cursor)? as usize;
        let mut name_bytes = vec![0; name_len];
        cursor
            .read_exact(&mut name_bytes)
            .map_err(|err| err.to_string())?;
        cursor
            .seek(SeekFrom::Current((extra_len + comment_len) as i64))
            .map_err(|err| err.to_string())?;
        if compression != 0 {
            return Err("Unsupported .mne zip compression".into());
        }
        if compressed_size != uncompressed_size {
            return Err("Invalid .mne zip size mismatch".into());
        }
        let name = String::from_utf8(name_bytes).map_err(|err| err.to_string())?;
        validate_bundle_path(&name)?;
        let data = read_local_zip_entry(bytes, local_offset, compressed_size)?;
        if crc32(&data) != crc {
            return Err("Invalid .mne zip CRC".into());
        }
        entries.insert(name, data);
    }
    Ok(entries)
}

pub(crate) fn read_local_zip_entry(
    bytes: &[u8],
    offset: usize,
    size: usize,
) -> Result<Vec<u8>, String> {
    let mut cursor = Cursor::new(bytes);
    cursor
        .seek(SeekFrom::Start(offset as u64))
        .map_err(|err| err.to_string())?;
    if read_u32(&mut cursor)? != 0x0403_4b50 {
        return Err("Invalid .mne zip local header".into());
    }
    cursor
        .seek(SeekFrom::Current(22))
        .map_err(|err| err.to_string())?;
    let name_len = read_u16(&mut cursor)? as u64;
    let extra_len = read_u16(&mut cursor)? as u64;
    cursor
        .seek(SeekFrom::Current((name_len + extra_len) as i64))
        .map_err(|err| err.to_string())?;
    let mut data = vec![0; size];
    cursor
        .read_exact(&mut data)
        .map_err(|err| err.to_string())?;
    Ok(data)
}

pub(crate) fn write_u16<W: Write>(writer: &mut W, value: u16) -> Result<(), String> {
    writer
        .write_all(&value.to_le_bytes())
        .map_err(|err| err.to_string())
}

pub(crate) fn write_u32<W: Write>(writer: &mut W, value: u32) -> Result<(), String> {
    writer
        .write_all(&value.to_le_bytes())
        .map_err(|err| err.to_string())
}

pub(crate) fn read_u16<R: Read>(reader: &mut R) -> Result<u16, String> {
    let mut bytes = [0; 2];
    reader
        .read_exact(&mut bytes)
        .map_err(|err| err.to_string())?;
    Ok(u16::from_le_bytes(bytes))
}

pub(crate) fn read_u32<R: Read>(reader: &mut R) -> Result<u32, String> {
    let mut bytes = [0; 4];
    reader
        .read_exact(&mut bytes)
        .map_err(|err| err.to_string())?;
    Ok(u32::from_le_bytes(bytes))
}

pub(crate) fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for byte in bytes {
        crc ^= *byte as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}
