use ::std::fmt::Display;
use ::std::io;
#[derive(Eq, PartialEq)]
pub struct MonoPcm16WaveFile {
    pub sample_rate: u32,
    pub samples: Vec<i16>,
}
#[derive(Debug)]
pub enum WavError {
    NotAWaveFile,
    WaveFileTooLarge,
    NoSamples,
    NotAPcmWaveFile,
    InvalidBlockAlign,
    Not8Or16BitPcmWav,
    NotAMonoWavFile,
    InvalidWaveFile,
    InvaidDataChunkSize,
    IoError(io::Error),
}
impl Display for WavError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WavError::NotAWaveFile => write!(f, "not a .wav file"),
            WavError::WaveFileTooLarge => write!(f, "wave file is too large"),
            WavError::NoSamples => write!(f, "wave file is empty (no samples)"),
            WavError::NotAPcmWaveFile => write!(f, "not a PCM (uncompressed) wave file"),
            WavError::InvalidBlockAlign => {
                write!(f, "invalid wave file header (block align)")
            }
            WavError::Not8Or16BitPcmWav => {
                write!(f, "not an 8-bit or 16-bit mono PCM wave file")
            }
            WavError::NotAMonoWavFile => write!(f, "not a mono wave file"),
            WavError::InvalidWaveFile => write!(f, "invalid wave file"),
            WavError::InvaidDataChunkSize => {
                write!(f, "invalid wave file: invalid data chunk size")
            }
            WavError::IoError(io_error) => io_error.fmt(f),
        }
    }
}
impl From<std::io::Error> for WavError {
    fn from(e: io::Error) -> Self {
        Self::IoError(e)
    }
}
const WAV_FORMAT_PCM_FORMAT: u16 = 1;
#[derive(Debug, Eq, PartialEq)]
struct FmtChunk {
    format_tag: u16,
    n_channels: u16,
    samples_per_second: u32,
    avg_bytes_per_second: u32,
    block_align: u16,
    bits_per_sample: u16,
}
#[derive(Debug, Eq, PartialEq)]
struct WaveFile {
    format: FmtChunk,
    data: Vec<u8>,
}
const WAVE_CHUNK_ID: [u8; 4] = *b"RIFF";
const WAVE_ID: [u8; 4] = *b"WAVE";
const FMT_CHUNK_ID: [u8; 4] = *b"fmt ";
const DATA_CHUNK_ID: [u8; 4] = *b"data";
fn parse_fmt_chunk(data: &[u8]) -> Result<FmtChunk, WavError> {
    if data.len() < 16 {
        return Err(WavError::InvalidWaveFile);
    }
    let read_u16 = |o: usize| -> u16 { u16::from_le_bytes([data[o], data[o + 1]]) };
    let read_u32 =
        |o: usize| -> u32 { u32::from_le_bytes([data[o], data[o + 1], data[o + 2], data[o + 3]]) };
    let format_tag = read_u16(0);
    let n_channels = read_u16(2);
    let samples_per_second = read_u32(4);
    let avg_bytes_per_second = read_u32(8);
    let block_align = read_u16(12);
    let bits_per_sample = read_u16(14);
    Ok(FmtChunk {
        format_tag,
        n_channels,
        samples_per_second,
        avg_bytes_per_second,
        block_align,
        bits_per_sample,
    })
}
fn read_four_bytes(reader: &mut impl io::Read) -> io::Result<[u8; 4]> {
    let mut data = [0; 4];
    reader.read_exact(&mut data)?;
    Ok(data)
}
fn read_wave_file(
    reader: &mut (impl io::Read + io::Seek),
    max_data_size: usize,
) -> Result<WaveFile, WavError> {
    if read_four_bytes(reader)? != WAVE_CHUNK_ID {
        return Err(WavError::NotAWaveFile);
    }
    let _file_size = read_four_bytes(reader)?;
    if read_four_bytes(reader)? != WAVE_ID {
        return Err(WavError::NotAWaveFile);
    }
    let format = {
        let chunk_id = read_four_bytes(reader)?;
        let chunk_size = u32::from_le_bytes(read_four_bytes(reader)?);
        if chunk_id != FMT_CHUNK_ID {
            return Err(WavError::InvalidWaveFile);
        }
        if chunk_size >= 100 {
            return Err(WavError::InvalidWaveFile);
        }
        let chunk_data = {
            let mut v = vec![0; chunk_size.try_into().unwrap()];
            reader.read_exact(&mut v)?;
            v
        };
        parse_fmt_chunk(chunk_data.as_slice())?
    };
    let mut data = Vec::new();
    loop {
        let mut chunk_id = [0; 4];
        let n = reader.read(&mut chunk_id)?;
        if n == 0 {
            break;
        }
        if n != chunk_id.len() {
            return Err(WavError::InvalidWaveFile);
        }
        let chunk_size = u32::from_le_bytes(read_four_bytes(reader)?);
        match chunk_id {
            DATA_CHUNK_ID => {
                let start = data.len();
                let chunk_size = usize::try_from(chunk_size).unwrap();
                if data.len() + chunk_size > max_data_size {
                    return Err(WavError::WaveFileTooLarge);
                }
                data.resize(data.len() + chunk_size, 0);
                reader.read_exact(&mut data[start..])?;
            }
            FMT_CHUNK_ID => {
                return Err(WavError::InvalidWaveFile);
            }
            _ => {
                reader.seek(io::SeekFrom::Current(chunk_size.into()))?;
            }
        }
    }
    if data.is_empty() {
        return Err(WavError::NoSamples);
    }
    Ok(WaveFile { format, data })
}
fn decode_16_bit(wav: WaveFile) -> Result<MonoPcm16WaveFile, WavError> {
    if wav.data.len() % 2 != 0 {
        return Err(WavError::InvaidDataChunkSize);
    }
    let mut samples = Vec::with_capacity(wav.data.len() / 2);
    samples.extend(
        wav.data
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]])),
    );
    Ok(MonoPcm16WaveFile {
        sample_rate: wav.format.samples_per_second,
        samples,
    })
}
fn decode_8_bit(wav: WaveFile) -> Result<MonoPcm16WaveFile, WavError> {
    let samples = wav
        .data
        .iter()
        .map(|s| (i16::from(*s) - 128) << 8)
        .collect();
    Ok(MonoPcm16WaveFile {
        sample_rate: wav.format.samples_per_second,
        samples,
    })
}
pub fn read_mono_pcm_wave_file(
    reader: &mut (impl io::Read + io::Seek),
    max_samples: usize,
) -> Result<MonoPcm16WaveFile, WavError> {
    let wav = read_wave_file(reader, max_samples * 2)?;
    let fmt = &wav.format;
    if fmt.format_tag != WAV_FORMAT_PCM_FORMAT {
        Err(WavError::NotAPcmWaveFile)
    } else if fmt.block_align != fmt.n_channels * fmt.bits_per_sample / 8 {
        Err(WavError::InvalidBlockAlign)
    } else {
        match (fmt.bits_per_sample, fmt.n_channels) {
            (16, 1) => decode_16_bit(wav),
            (8, 1) => decode_8_bit(wav),
            (16, _) => Err(WavError::NotAMonoWavFile),
            (8, _) => Err(WavError::NotAMonoWavFile),
            (_, _) => Err(WavError::Not8Or16BitPcmWav),
        }
    }
}
