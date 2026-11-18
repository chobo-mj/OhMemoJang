//! Korean public holidays, computed offline.
//!
//! - Solar holidays come from fixed dates.
//! - The lunar ones (설날, 부처님오신날, 추석) come from a Korean lunisolar
//!   calendar worked out here from the positions of the Sun and the Moon:
//!   new moons (Meeus, *Astronomical Algorithms*, ch. 49) and the Sun's
//!   longitude (ch. 25), both reckoned in Korean time (UTC+9).
//! - Substitute holidays (대체공휴일), the regular election days, and the past
//!   temporary holidays (임시공휴일) are added on top.
//!
//! The rules follow 「관공서의 공휴일에 관한 규정」 and 「공휴일에 관한
//! 법률」 as of 2026, including 노동절 and 제헌절 becoming public holidays
//! again. Holidays the government declares on short notice (임시공휴일, early
//! elections) can't be foreseen; the ones known up to 2025 are listed.
use chrono::{Datelike, Duration, NaiveDate, Weekday};
use std::collections::HashMap;
use std::sync::Mutex;

/// One holiday on a date.
#[derive(Clone, Debug, PartialEq)]
pub struct Holiday {
    pub date: NaiveDate,
    pub name: &'static str,
    /// A 대체공휴일 rather than the holiday itself.
    pub substitute: bool,
}

/// The names of the holidays on `date` ("어린이날 · 부처님오신날"), if any.
pub fn name(date: NaiveDate) -> Option<String> {
    let names: Vec<&str> = for_year(date.year())
        .into_iter()
        .filter(|h| h.date == date)
        .map(|h| h.name)
        .collect();
    (!names.is_empty()).then(|| names.join(" · "))
}

/// True when `date` is a public holiday (substitutes included).
#[cfg(test)]
pub fn is_holiday(date: NaiveDate) -> bool {
    for_year(date.year()).iter().any(|h| h.date == date)
}

static CACHE: Mutex<Option<HashMap<i32, Vec<Holiday>>>> = Mutex::new(None);

/// Every holiday of `year`, in date order. Worked out once per year.
pub fn for_year(year: i32) -> Vec<Holiday> {
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    cache
        .get_or_insert_with(HashMap::new)
        .entry(year)
        .or_insert_with(|| compute(year))
        .clone()
}

// ---------------------------------------------------------------------------
// Astronomy

const J2000: f64 = 2_451_545.0;

fn sin_d(x: f64) -> f64 {
    x.to_radians().sin()
}

/// Seconds between Terrestrial Time and Universal Time around `year`.
fn delta_t(year: f64) -> f64 {
    let t = year - 2000.;
    if (2005.0..2050.).contains(&year) {
        62.92 + 0.32217 * t + 0.005589 * t * t
    } else if (1986.0..2005.).contains(&year) {
        63.86 + 0.3345 * t - 0.060374 * t * t
            + 0.0017275 * t.powi(3)
            + 0.000651814 * t.powi(4)
            + 0.00002373599 * t.powi(5)
    } else {
        let u = (year - 1820.) / 100.;
        -20. + 32. * u * u
    }
}

/// Julian day (TT) of the new moon of lunation `k` (k = 0: January 2000).
fn new_moon_jde(k: f64) -> f64 {
    let t = k / 1236.85;
    let t2 = t * t;
    let t3 = t2 * t;
    let t4 = t3 * t;
    let jde = 2_451_550.097_66 + 29.530_588_861 * k + 0.000_154_37 * t2 - 0.000_000_150 * t3
        + 0.000_000_000_73 * t4;
    let e = 1. - 0.002_516 * t - 0.000_007_4 * t2;
    let m = 2.5534 + 29.105_356_70 * k - 0.000_001_4 * t2 - 0.000_000_11 * t3;
    let mp =
        201.5643 + 385.816_935_28 * k + 0.010_758_2 * t2 + 0.000_012_38 * t3 - 0.000_000_058 * t4;
    let f =
        160.7108 + 390.670_502_84 * k - 0.001_611_8 * t2 - 0.000_002_27 * t3 + 0.000_000_011 * t4;
    let om = 124.7746 - 1.563_755_88 * k + 0.002_067_2 * t2 + 0.000_002_15 * t3;
    let c = -0.40720 * sin_d(mp)
        + 0.17241 * e * sin_d(m)
        + 0.01608 * sin_d(2. * mp)
        + 0.01039 * sin_d(2. * f)
        + 0.00739 * e * sin_d(mp - m)
        - 0.00514 * e * sin_d(mp + m)
        + 0.00208 * e * e * sin_d(2. * m)
        - 0.00111 * sin_d(mp - 2. * f)
        - 0.00057 * sin_d(mp + 2. * f)
        + 0.00056 * e * sin_d(2. * mp + m)
        - 0.00042 * sin_d(3. * mp)
        + 0.00042 * e * sin_d(m + 2. * f)
        + 0.00038 * e * sin_d(m - 2. * f)
        - 0.00024 * e * sin_d(2. * mp - m)
        - 0.00017 * sin_d(om)
        - 0.00007 * sin_d(mp + 2. * m)
        + 0.00004 * sin_d(2. * mp - 2. * f)
        + 0.00004 * sin_d(3. * m)
        + 0.00003 * sin_d(mp + m - 2. * f)
        + 0.00003 * sin_d(2. * mp + 2. * f)
        - 0.00003 * sin_d(mp + m + 2. * f)
        + 0.00003 * sin_d(mp - m + 2. * f)
        - 0.00002 * sin_d(mp - m - 2. * f)
        - 0.00002 * sin_d(3. * mp + m)
        + 0.00002 * sin_d(4. * mp);
    let a = [
        (0.000_325, 299.77 + 0.107_408 * k - 0.009_173 * t2),
        (0.000_165, 251.88 + 0.016_321 * k),
        (0.000_164, 251.83 + 26.651_886 * k),
        (0.000_126, 349.42 + 36.412_478 * k),
        (0.000_110, 84.66 + 18.206_239 * k),
        (0.000_062, 141.74 + 53.303_771 * k),
        (0.000_060, 207.14 + 2.453_732 * k),
        (0.000_056, 154.84 + 7.306_860 * k),
        (0.000_047, 34.52 + 27.261_239 * k),
        (0.000_042, 207.19 + 0.121_824 * k),
        (0.000_040, 291.34 + 1.844_379 * k),
        (0.000_037, 161.72 + 24.198_154 * k),
        (0.000_035, 239.56 + 25.513_099 * k),
        (0.000_023, 331.55 + 3.592_518 * k),
    ];
    jde + c + a.iter().map(|(w, x)| w * sin_d(*x)).sum::<f64>()
}

/// The Sun's apparent longitude in degrees at Julian day `jde` (TT).
fn sun_longitude(jde: f64) -> f64 {
    let t = (jde - J2000) / 36525.;
    let l0 = 280.466_46 + 36_000.769_83 * t + 0.000_303_2 * t * t;
    let m = 357.529_11 + 35_999.050_29 * t - 0.000_153_7 * t * t;
    let c = (1.914_602 - 0.004_817 * t - 0.000_014 * t * t) * sin_d(m)
        + (0.019_993 - 0.000_101 * t) * sin_d(2. * m)
        + 0.000_289 * sin_d(3. * m);
    let om = 125.04 - 1934.136 * t;
    (l0 + c - 0.005_69 - 0.004_78 * sin_d(om)).rem_euclid(360.)
}

/// Julian day (TT) when the Sun reaches `target` degrees, near `around`.
fn solar_term_jde(target: f64, around: f64) -> f64 {
    let mut jd = around;
    for _ in 0..50 {
        let diff = (target - sun_longitude(jd) + 540.).rem_euclid(360.) - 180.;
        jd += diff * 365.2422 / 360.;
        if diff.abs() < 1e-7 {
            break;
        }
    }
    jd
}

/// The date in Korea (UTC+9) of an instant given in Julian days (TT).
fn korean_date(jde: f64) -> NaiveDate {
    let year = 2000. + (jde - J2000) / 365.25;
    let ut = jde - delta_t(year) / 86_400.;
    let local = ut + 9. / 24.;
    let days = (local + 0.5).floor() as i64;
    // Julian day number 2440588 is 1970-01-01.
    NaiveDate::from_ymd_opt(1970, 1, 1).unwrap() + Duration::days(days - 2_440_588)
}

fn julian_day(date: NaiveDate) -> f64 {
    (date - NaiveDate::from_ymd_opt(1970, 1, 1).unwrap()).num_days() as f64 + 2_440_587.5
}

/// The first day (in Korea) of the lunar month in which `date` falls.
fn month_start(date: NaiveDate) -> NaiveDate {
    let jd = julian_day(date);
    let mut k = ((jd - 2_451_550.1) / 29.530_588_861).floor();
    loop {
        let start = korean_date(new_moon_jde(k));
        if start > date {
            k -= 1.;
        } else if korean_date(new_moon_jde(k + 1.)) <= date {
            k += 1.;
        } else {
            return start;
        }
    }
}

/// The next new moon (in Korea) after the month that starts on `start`.
fn next_month_start(start: NaiveDate) -> NaiveDate {
    month_start(start + Duration::days(35))
}

/// Date (in Korea) of the winter solstice of `year`.
fn winter_solstice(year: i32) -> NaiveDate {
    let guess = julian_day(NaiveDate::from_ymd_opt(year, 12, 21).unwrap());
    korean_date(solar_term_jde(270., guess))
}

/// The months of the lunisolar year from the 11th month that holds the
/// winter solstice of `year - 1` up to the next one: (first day, month
/// number, leap).
fn lunar_months(year: i32) -> Vec<(NaiveDate, u32, bool)> {
    let first = month_start(winter_solstice(year - 1));
    let last = month_start(winter_solstice(year));
    let mut starts = vec![first];
    while *starts.last().unwrap() < last {
        let next = next_month_start(*starts.last().unwrap());
        starts.push(next);
    }
    // `starts` runs from the 11th month to the next 11th month.
    let months = starts.len() - 1;
    // With 13 months in between, the first one holding no principal term
    // (the Sun crossing a multiple of 30°) is a leap month.
    let mut leap = None;
    if months == 13 {
        let terms: Vec<NaiveDate> = (0..14)
            .map(|n| {
                let around = julian_day(first) + n as f64 * 30.44;
                let target = ((270 + 30 * n) % 360) as f64;
                korean_date(solar_term_jde(target, around))
            })
            .collect();
        leap = (1..months).find(|&i| {
            let (from, to) = (starts[i], starts[i + 1]);
            !terms.iter().any(|d| *d >= from && *d < to)
        });
    }
    let mut out = vec![];
    let mut number = 11;
    for (i, start) in starts.iter().take(months).enumerate() {
        let is_leap = leap == Some(i);
        if i > 0 && !is_leap {
            number = number % 12 + 1;
        }
        out.push((*start, number, is_leap));
    }
    out
}

/// The solar date of `month`/`day` of the lunar year that begins in
/// Gregorian `year` (not a leap month).
pub fn lunar_to_solar(year: i32, month: u32, day: u32) -> Option<NaiveDate> {
    let months = lunar_months(year);
    // Months 11 and 12 at the start belong to the year before.
    let from = months.iter().position(|m| m.1 == 1 && !m.2)?;
    months[from..]
        .iter()
        .find(|m| m.1 == month && !m.2)
        .map(|m| m.0 + Duration::days(day as i64 - 1))
}

// ---------------------------------------------------------------------------
// Holidays

fn ymd(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

/// How a holiday is replaced when it is lost.
#[derive(Clone, Copy, PartialEq)]
enum Sub {
    None,
    /// Lost to a Saturday, a Sunday or another holiday (since `from`).
    Weekend(NaiveDate),
    /// 설날·추석: lost to a Sunday or another holiday; the substitute comes
    /// after the whole three days.
    Sunday(NaiveDate, NaiveDate),
}

/// Temporary holidays the government declared (not computable).
const TEMPORARY: &[(i32, u32, u32, &str)] = &[
    (2002, 7, 1, "임시공휴일"),
    (2015, 8, 14, "임시공휴일"),
    (2016, 5, 6, "임시공휴일"),
    (2017, 5, 9, "대통령 선거일"),
    (2017, 10, 2, "임시공휴일"),
    (2020, 8, 17, "임시공휴일"),
    (2023, 10, 2, "임시공휴일"),
    (2024, 10, 1, "국군의 날 임시공휴일"),
    (2025, 1, 27, "임시공휴일"),
    (2025, 6, 3, "대통령 선거일"),
];

/// End of the president's term before each regular presidential election;
/// after the last one listed, every five years.
const PRESIDENT_TERMS: &[(i32, u32, u32)] =
    &[(2008, 2, 24), (2013, 2, 24), (2022, 5, 9), (2030, 6, 3)];

fn add(list: &mut Vec<(Holiday, Sub)>, date: NaiveDate, name: &'static str, sub: Sub) {
    list.push((
        Holiday {
            date,
            name,
            substitute: false,
        },
        sub,
    ));
}

fn compute(year: i32) -> Vec<Holiday> {
    let mut list: Vec<(Holiday, Sub)> = vec![];
    let weekend = |y: i32, m: u32, d: u32| Sub::Weekend(ymd(y, m, d));
    add(&mut list, ymd(year, 1, 1), "신정", Sub::None);
    if let Some(new_year) = lunar_to_solar(year, 1, 1) {
        let last = new_year + Duration::days(1);
        let sub = Sub::Sunday(ymd(2014, 1, 1), last);
        add(&mut list, new_year - Duration::days(1), "설날 연휴", sub);
        add(&mut list, new_year, "설날", sub);
        add(&mut list, last, "설날 연휴", sub);
    }
    add(&mut list, ymd(year, 3, 1), "삼일절", weekend(2021, 8, 15));
    if year <= 2005 {
        add(&mut list, ymd(year, 4, 5), "식목일", Sub::None);
    }
    if let Some(buddha) = lunar_to_solar(year, 4, 8) {
        add(&mut list, buddha, "부처님오신날", weekend(2023, 5, 4));
    }
    if year >= 2026 {
        add(&mut list, ymd(year, 5, 1), "노동절", weekend(2026, 5, 1));
    }
    add(&mut list, ymd(year, 5, 5), "어린이날", weekend(2014, 1, 1));
    add(&mut list, ymd(year, 6, 6), "현충일", Sub::None);
    if year <= 2007 || year >= 2026 {
        add(&mut list, ymd(year, 7, 17), "제헌절", weekend(2026, 7, 17));
    }
    add(&mut list, ymd(year, 8, 15), "광복절", weekend(2021, 8, 15));
    if let Some(chuseok) = lunar_to_solar(year, 8, 15) {
        let last = chuseok + Duration::days(1);
        let sub = Sub::Sunday(ymd(2014, 1, 1), last);
        add(&mut list, chuseok - Duration::days(1), "추석 연휴", sub);
        add(&mut list, chuseok, "추석", sub);
        add(&mut list, last, "추석 연휴", sub);
    }
    add(&mut list, ymd(year, 10, 3), "개천절", weekend(2021, 8, 15));
    if year <= 1990 || year >= 2013 {
        add(&mut list, ymd(year, 10, 9), "한글날", weekend(2021, 8, 15));
    }
    add(&mut list, ymd(year, 12, 25), "성탄절", weekend(2023, 5, 4));
    for &(y, m, d, name) in TEMPORARY {
        if y == year {
            add(&mut list, ymd(y, m, d), name, Sub::None);
        }
    }
    // Election days are set clear of the holidays above.
    let base: Vec<NaiveDate> = list.iter().map(|(h, _)| h.date).collect();
    for (date, name) in elections(year, &base) {
        add(&mut list, date, name, Sub::None);
    }

    // Substitute holidays. A holiday is "lost" on a Sunday (any kind that
    // has a substitute), on a Saturday (the Weekend kind), or when it shares
    // its day with another holiday (all but one of them are lost).
    let holiday_on = |d: NaiveDate, list: &[(Holiday, Sub)]| list.iter().any(|(h, _)| h.date == d);
    let mut dates: Vec<NaiveDate> = list.iter().map(|(h, _)| h.date).collect();
    dates.sort();
    dates.dedup();
    let mut substitutes: Vec<NaiveDate> = vec![];
    for day in dates {
        let here: Vec<Sub> = list
            .iter()
            .filter(|(h, _)| h.date == day)
            .map(|(_, s)| *s)
            .collect();
        let eligible: Vec<(Sub, NaiveDate)> = here
            .iter()
            .filter_map(|s| match s {
                Sub::Weekend(from) if day >= *from => Some((*s, day)),
                Sub::Sunday(from, last) if day >= *from => Some((*s, *last)),
                _ => None,
            })
            .collect();
        if eligible.is_empty() {
            continue;
        }
        let lost = match day.weekday() {
            Weekday::Sun => eligible.len(),
            Weekday::Sat => eligible
                .iter()
                .filter(|(s, _)| matches!(s, Sub::Weekend(_)))
                .count()
                .max(eligible.len().min(here.len() - 1)),
            _ => eligible.len().min(here.len() - 1),
        };
        for (_, after) in eligible.iter().take(lost) {
            let mut d = *after + Duration::days(1);
            while matches!(d.weekday(), Weekday::Sat | Weekday::Sun)
                || holiday_on(d, &list)
                || substitutes.contains(&d)
            {
                d += Duration::days(1);
            }
            substitutes.push(d);
        }
    }
    let mut out: Vec<Holiday> = list.into_iter().map(|(h, _)| h).collect();
    out.extend(substitutes.into_iter().map(|date| Holiday {
        date,
        name: "대체공휴일",
        substitute: true,
    }));
    // 설날 eve is never before January 20 and 성탄절 substitutes stay in
    // December, so no substitute crosses into another year.
    out.sort_by_key(|h| h.date);
    out
}

/// Regular election days of `year` (presidential, National Assembly, local).
fn elections(year: i32, holidays: &[NaiveDate]) -> Vec<(NaiveDate, &'static str)> {
    // The first Wednesday on or after `days` before the end of the term; a
    // Wednesday that is a holiday, or next to one, moves a week on.
    let rule = |term_end: NaiveDate, days: i64| {
        let mut d = term_end - Duration::days(days);
        while d.weekday() != Weekday::Wed {
            d += Duration::days(1);
        }
        while [d - Duration::days(1), d, d + Duration::days(1)]
            .iter()
            .any(|x| holidays.contains(x))
        {
            d += Duration::days(7);
        }
        d
    };
    let mut out = vec![];
    if year >= 2008 && (year - 2008) % 4 == 0 {
        out.push((rule(ymd(year, 5, 29), 50), "국회의원 선거일"));
    }
    if year >= 2006 && (year - 2006) % 4 == 0 {
        out.push((rule(ymd(year, 6, 30), 30), "지방선거일"));
    }
    let mut terms: Vec<NaiveDate> = PRESIDENT_TERMS
        .iter()
        .map(|&(y, m, d)| ymd(y, m, d))
        .collect();
    let mut next = *terms.last().unwrap();
    while next.year() < year + 2 {
        next = ymd(next.year() + 5, next.month(), next.day());
        terms.push(next);
    }
    for end in terms {
        let date = rule(end, 70);
        if date.year() == year {
            out.push((date, "대통령 선거일"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn lunar_new_year_chuseok_and_buddhas_birthday() {
        let seollal = [
            "2000-02-05",
            "2001-01-24",
            "2002-02-12",
            "2003-02-01",
            "2004-01-22",
            "2005-02-09",
            "2006-01-29",
            "2007-02-18",
            "2008-02-07",
            "2009-01-26",
            "2010-02-14",
            "2011-02-03",
            "2012-01-23",
            "2013-02-10",
            "2014-01-31",
            "2015-02-19",
            "2016-02-08",
            "2017-01-28",
            "2018-02-16",
            "2019-02-05",
            "2020-01-25",
            "2021-02-12",
            "2022-02-01",
            "2023-01-22",
            "2024-02-10",
            "2025-01-29",
            "2026-02-17",
            "2027-02-07",
            "2028-01-27",
            "2029-02-13",
            "2030-02-03",
        ];
        for s in seollal {
            let want = d(s);
            assert_eq!(lunar_to_solar(want.year(), 1, 1), Some(want), "설날 {s}");
        }
        let chuseok = [
            "2015-09-27",
            "2016-09-15",
            "2017-10-04",
            "2018-09-24",
            "2019-09-13",
            "2020-10-01",
            "2021-09-21",
            "2022-09-10",
            "2023-09-29",
            "2024-09-17",
            "2025-10-06",
            "2026-09-25",
            "2027-09-15",
            "2028-10-03",
            "2029-09-22",
            "2030-09-12",
        ];
        for s in chuseok {
            let want = d(s);
            assert_eq!(lunar_to_solar(want.year(), 8, 15), Some(want), "추석 {s}");
        }
        let buddha = [
            "2015-05-25",
            "2016-05-14",
            "2017-05-03",
            "2018-05-22",
            "2019-05-12",
            "2020-04-30",
            "2021-05-19",
            "2022-05-08",
            "2023-05-27",
            "2024-05-15",
            "2025-05-05",
            "2026-05-24",
            "2027-05-13",
            "2028-05-02",
            "2029-05-20",
            "2030-05-09",
        ];
        for s in buddha {
            let want = d(s);
            assert_eq!(
                lunar_to_solar(want.year(), 4, 8),
                Some(want),
                "부처님오신날 {s}"
            );
        }
    }

    #[test]
    fn leap_months_and_a_sane_calendar_for_two_centuries() {
        // (year, leap month) of recent Korean leap years.
        for (year, month) in [
            (2012, 3),
            (2014, 9),
            (2017, 5),
            (2020, 4),
            (2023, 2),
            (2025, 6),
            (2028, 5),
        ] {
            let months = lunar_months(year + if month >= 11 { 1 } else { 0 });
            let leap: Vec<u32> = months.iter().filter(|m| m.2).map(|m| m.1).collect();
            assert_eq!(leap, vec![month], "{year}");
        }
        for year in 1950..=2100 {
            let new_year = lunar_to_solar(year, 1, 1).unwrap();
            assert!(
                new_year >= d(&format!("{year}-01-21")) && new_year <= d(&format!("{year}-02-20")),
                "{year} {new_year}"
            );
            let chuseok = lunar_to_solar(year, 8, 15).unwrap();
            assert!(
                chuseok >= d(&format!("{year}-09-07")) && chuseok <= d(&format!("{year}-10-08")),
                "{year} {chuseok}"
            );
            let months = lunar_months(year);
            assert!(months.len() == 12 || months.len() == 13);
            assert_eq!(months.iter().filter(|m| m.2).count(), months.len() - 12);
            for w in months.windows(2) {
                let len = (w[1].0 - w[0].0).num_days();
                assert!(len == 29 || len == 30, "{year}");
            }
        }
    }

    fn substitutes(year: i32) -> Vec<NaiveDate> {
        compute(year)
            .into_iter()
            .filter(|h| h.substitute)
            .map(|h| h.date)
            .collect()
    }

    #[test]
    fn substitute_holidays_follow_the_rules_of_each_year() {
        let known: &[(i32, &[&str])] = &[
            (2013, &[]),
            (2014, &["2014-09-10"]),
            (2015, &["2015-09-29"]),
            (2016, &["2016-02-10"]),
            (2017, &["2017-01-30", "2017-10-06"]),
            (2018, &["2018-05-07", "2018-09-26"]),
            (2019, &["2019-05-06"]),
            (2020, &["2020-01-27"]),
            (2021, &["2021-08-16", "2021-10-04", "2021-10-11"]),
            (2022, &["2022-09-12", "2022-10-10"]),
            (2023, &["2023-01-24", "2023-05-29"]),
            (2024, &["2024-02-12", "2024-05-06"]),
            (2025, &["2025-03-03", "2025-05-06", "2025-10-08"]),
            (
                2026,
                &["2026-03-02", "2026-05-25", "2026-08-17", "2026-10-05"],
            ),
            (
                2027,
                &[
                    "2027-02-09",
                    "2027-05-03",
                    "2027-07-19",
                    "2027-08-16",
                    "2027-10-04",
                    "2027-10-11",
                    "2027-12-27",
                ],
            ),
        ];
        for (year, want) in known {
            let want: Vec<NaiveDate> = want.iter().map(|s| d(s)).collect();
            assert_eq!(substitutes(*year), want, "{year}");
        }
    }

    #[test]
    fn election_days_and_new_holidays() {
        for (s, want) in [
            ("2006-05-31", "지방선거일"),
            ("2007-12-19", "대통령 선거일"),
            ("2008-04-09", "국회의원 선거일"),
            ("2010-06-02", "지방선거일"),
            ("2012-04-11", "국회의원 선거일"),
            ("2012-12-19", "대통령 선거일"),
            ("2014-06-04", "지방선거일"),
            ("2016-04-13", "국회의원 선거일"),
            ("2018-06-13", "지방선거일"),
            ("2020-04-15", "국회의원 선거일"),
            ("2022-03-09", "대통령 선거일"),
            ("2022-06-01", "지방선거일"),
            ("2024-04-10", "국회의원 선거일"),
            ("2026-06-03", "지방선거일"),
            ("2028-04-12", "국회의원 선거일"),
        ] {
            assert_eq!(name(d(s)).as_deref(), Some(want), "{s}");
        }
        assert_eq!(name(d("2026-05-01")).as_deref(), Some("노동절"));
        assert_eq!(name(d("2026-07-17")).as_deref(), Some("제헌절"));
        assert_eq!(name(d("2025-07-17")), None);
        assert_eq!(name(d("2025-05-01")), None);
        assert_eq!(
            name(d("2025-05-05")).as_deref(),
            Some("부처님오신날 · 어린이날")
        );
        assert!(is_holiday(d("2026-02-16")) && is_holiday(d("2026-02-18")));
        assert!(!is_holiday(d("2026-02-19")));
    }
}
