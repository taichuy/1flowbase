//! Version-3 anchors locate an exact CAD1 directory inside the authenticated block.
use anyhow::{ensure, Context, Result};

const MAGIC: &[u8; 4] = b"CAL1";

pub(super) fn append(block: &mut Vec<u8>, directory: &[u8]) -> Result<Vec<u8>> {
    ensure!(!directory.is_empty(), "archive packed directory empty");
    let mut locator = MAGIC.to_vec();
    put(&mut locator, block.len().try_into()?);
    put(&mut locator, directory.len().try_into()?);
    block.extend_from_slice(directory);
    Ok(locator)
}

pub(super) fn resolve<'a>(locator: &[u8], block: &'a [u8]) -> Result<&'a [u8]> {
    ensure!(
        locator.starts_with(MAGIC),
        "archive directory locator header invalid"
    );
    let mut input = &locator[MAGIC.len()..];
    let offset: usize = take(&mut input)?.try_into()?;
    let length: usize = take(&mut input)?.try_into()?;
    ensure!(
        input.is_empty() && length > 0,
        "archive directory locator shape invalid"
    );
    let end = offset
        .checked_add(length)
        .context("archive directory locator overflow")?;
    block
        .get(offset..end)
        .context("archive directory locator out of bounds")
}

fn put(output: &mut Vec<u8>, mut value: u64) {
    while value >= 128 {
        output.push((value as u8 & 127) | 128);
        value >>= 7;
    }
    output.push(value as u8);
}

fn take(input: &mut &[u8]) -> Result<u64> {
    let mut value = 0;
    for index in 0..10 {
        let (&byte, remaining) = input
            .split_first()
            .context("archive directory locator truncated")?;
        *input = remaining;
        ensure!(
            index < 9 || byte <= 1,
            "archive directory locator integer overflow"
        );
        value |= u64::from(byte & 127) << (index * 7);
        if byte & 128 == 0 {
            ensure!(
                index == 0 || byte != 0,
                "archive directory locator integer noncanonical"
            );
            return Ok(value);
        }
    }
    anyhow::bail!("archive directory locator integer overflow")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn locators_preserve_originals_and_reject_corruption() {
        let mut block = vec![0, 255, 128];
        let locator = append(&mut block, b"CAD1\0\xfforiginal").unwrap();
        assert_eq!(resolve(&locator, &block).unwrap(), b"CAD1\0\xfforiginal");
        for invalid in [
            b"bad".to_vec(),
            b"CAL1\x80\x00\x01".to_vec(),
            b"CAL1\x03\x00".to_vec(),
            b"CAL1\x03\x7f".to_vec(),
            [locator.clone(), vec![0]].concat(),
            locator[..locator.len() - 1].to_vec(),
        ] {
            assert!(resolve(&invalid, &block).is_err());
        }
    }
}
