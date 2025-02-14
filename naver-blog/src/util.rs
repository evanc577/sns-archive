use std::sync::LazyLock;

use jiff::tz::TimeZone;
use jiff::{RoundMode, ToSpan, Unit, Zoned, ZonedRound};
use regex::Regex;
use unicode_segmentation::UnicodeSegmentation;

pub(crate) fn parse_date(time: &str) -> Result<jiff::Zoned, String> {
    static RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^(?:(?<now>방금 *전)|(?<rel_min>\d+)분 *전|(?<rel_hr>\d+)시간 *전|(?<abs_yr>\d+)\. *(?<abs_mon>\d+)\. *(?<abs_day>\d+)\.(?: *\d+:\d+)?)$").unwrap()
    });
    static TZ: &str = "Asia/Seoul";

    let now = Zoned::now().with_time_zone(TimeZone::get(TZ).unwrap());
    let time = RE
        .captures(time)
        .and_then(|cap| {
            if cap.name("now").is_some() {
                // Just now
                Some(now)
            } else if let Some(min) = cap.name("rel_min") {
                // X minutes ago
                min.as_str()
                    .parse::<i64>()
                    .ok()
                    .map(|min| now.saturating_sub(min.minutes()))
            } else if let Some(hr) = cap.name("rel_hr") {
                // X hours ago
                hr.as_str()
                    .parse::<i64>()
                    .ok()
                    .map(|hr| now.saturating_sub(hr.hours()))
            } else if let (Some(yr), Some(mon), Some(day)) =
                (cap.name("abs_yr"), cap.name("abs_mon"), cap.name("abs_day"))
            {
                // Absolute date + time
                let time = Zoned::strptime(
                    "%Y %-m %-d %Q",
                    format!("{} {} {} {}", yr.as_str(), mon.as_str(), day.as_str(), TZ,),
                )
                .unwrap();
                Some(time)
            } else {
                None
            }
        })
        .and_then(|t| {
            t.round(ZonedRound::new().smallest(Unit::Day).mode(RoundMode::Trunc))
                .ok()
        })
        .ok_or("unable to parse time".to_owned())?;
    Ok(time)
}

#[derive(Debug)]
pub(crate) struct NaverBlogMetadata {
    pub member: String,
    pub post_id: u64,
    pub title: String,
    pub time: jiff::Zoned,
}

pub(crate) fn slug(metadata: &NaverBlogMetadata) -> String {
    let trunc_title: String = UnicodeSegmentation::graphemes(metadata.title.as_str(), true)
        .take(50)
        .collect();

    let slug = format!(
        "{}-{}-{}-{}",
        metadata.time.strftime("%Y%m%d"),
        &metadata.member,
        metadata.post_id,
        trunc_title,
    );
    let sanitize_options = sanitize_filename::Options {
        windows: true,
        ..Default::default()
    };
    
    sanitize_filename::sanitize_with_options(slug, sanitize_options)
}

#[cfg(test)]
mod test {
    use super::parse_date;

    #[test]
    fn test_parse_time() {
        // Absolute
        parse_date("2025. 1. 16. 15:00").unwrap();
        parse_date("2025. 1. 16.").unwrap();

        // Relative hours
        parse_date("22시간 전").unwrap();

        // Relative minutes
        parse_date("1분 전").unwrap();

        // Just now
        parse_date("방금 전").unwrap();
    }
}
