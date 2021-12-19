use chrono::NaiveTime;
use nom::bytes::complete::take_until;
use nom::character::complete::{char, one_of};
use nom::combinator::{map, opt};
use nom::IResult;

use crate::parse::NmeaSentence;
use crate::{
    sentences::utils::{do_parse_lat_lon, parse_hms},
    NmeaError,
};

/// Parse GPGLL (Geographic position)
/// From https://docs.novatel.com/OEM7/Content/Logs/GPGLL.htm
///
/// | Field | Structure   | Description
/// |-------|-------------|---------------------------------------------------------------------
/// | 1     | $GPGLL      | Log header.
/// | 2     | lat         | Latitude (DDmm.mm)
/// | 3     | lat dir     | Latitude direction (N = North, S = South)
/// | 4     | lon         | Longitude (DDDmm.mm)
/// | 5     | lon dir     | Longitude direction (E = East, W = West)
/// | 6     | utc         | UTC time status of position (hours/minutes/seconds/decimal seconds)
/// | 7     | data status | Data status: A = Data valid, V = Data invalid
/// | 8     | mode ind    | Positioning system mode indicator, see `PosSystemIndicator`
/// | 9     | *xx         | Check sum
pub fn parse_gll(sentence: NmeaSentence) -> Result<GllData, NmeaError> {
    if sentence.message_id != b"GLL" {
        Err(NmeaError::WrongSentenceHeader {
            expected: b"GLL",
            found: sentence.message_id,
        })
    } else {
        Ok(do_parse_gll(sentence.data)?.1)
    }
}

/// Positioning System Mode Indicator (present from NMEA >= 2.3)
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum PosSystemIndicator {
    Autonomous,
    Differential,
    EstimatedMode,
    ManualInput,
    DataNotValid,
}

impl From<char> for PosSystemIndicator {
    fn from(b: char) -> Self {
        match b {
            'A' => PosSystemIndicator::Autonomous,
            'D' => PosSystemIndicator::Differential,
            'E' => PosSystemIndicator::EstimatedMode,
            'M' => PosSystemIndicator::ManualInput,
            'N' => PosSystemIndicator::DataNotValid,
            _ => PosSystemIndicator::DataNotValid,
        }
    }
}

/// This field works with combination with the indicator field.
///
/// Quote from the NMEA standard:
/// "The Status field shall be set to 'V' = Invalid for all values of
/// Indicator mode except for A = Autonomous and D = Differential."
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum GLLDataStatus {
    Valid,
    Invalid,
}

impl From<char> for GLLDataStatus {
    fn from(b: char) -> Self {
        match b {
            'A' => GLLDataStatus::Valid,
            _ => GLLDataStatus::Invalid,
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct GllData {
    pub latitude: f64,
    pub longitude: f64,
    pub fix_time: NaiveTime,
    pub data_state: GLLDataStatus,
    pub mode: Option<PosSystemIndicator>,
}

fn do_parse_gll(i: &[u8]) -> IResult<&[u8], GllData> {
    let (i, (latitude, longitude)) = do_parse_lat_lon(i)?;
    let (i, _) = char(',')(i)?;
    let (i, fix_time) = parse_hms(i)?;
    let (i, _) = take_until(",")(i)?; // decimal ignored
    let (i, _) = char(',')(i)?;
    let (i, data_state) = map(one_of("AV"), GLLDataStatus::from)(i)?; // A: valid, V: invalid
    let (i, _) = char(',')(i)?;
    let (i, mode) = opt(
        map(one_of("ADEM"), PosSystemIndicator::from), // ignore 'N' for invalid
    )(i)?;

    Ok((
        i,
        GllData {
            latitude,
            longitude,
            fix_time,
            data_state,
            mode,
        },
    ))
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse_nmea_sentence;
    use approx::assert_relative_eq;

    #[test]
    fn test_parse_gpgll() {
        let s = parse_nmea_sentence(
            b"$GPGLL,5107.0013414,N,11402.3279144,W,205412.00,A,A*73",
        ).unwrap();
        assert_eq!(s.checksum, s.calc_checksum());
        assert_eq!(s.checksum, 0x73);

        let gll_data = parse_gll(s).unwrap();
        assert_relative_eq!(gll_data.latitude, 51.0 + (7.0013414 / 60.0));
        assert_relative_eq!(gll_data.longitude, -(114.0 + (2.3279144 / 60.0)));
        assert_eq!(gll_data.fix_time, NaiveTime::from_hms_milli(20, 54, 12, 000));
        assert_eq!(gll_data.data_state, GLLDataStatus::Valid);
        assert_eq!(gll_data.mode, Some(PosSystemIndicator::Autonomous));
    }

    #[test]
    fn test_parse_void_gngll() {
        let s = parse_nmea_sentence(
            b"$GNGLL,5107.0014143,N,11402.3278489,W,205122.00,V,E*7D"
        ).unwrap();
        assert_eq!(s.checksum, s.calc_checksum());
        assert_eq!(s.checksum, 0x7D);

        let gll_data = parse_gll(s).unwrap();
        assert_relative_eq!(gll_data.latitude, 51.0 + (7.0014143 / 60.0));
        assert_relative_eq!(gll_data.longitude, -(114.0 + (2.3278489 / 60.0)));
        assert_eq!(gll_data.fix_time, NaiveTime::from_hms_milli(20, 51, 22, 000));
        assert_eq!(gll_data.data_state, GLLDataStatus::Invalid);
        assert_eq!(gll_data.mode, Some(PosSystemIndicator::EstimatedMode));
    }
}
