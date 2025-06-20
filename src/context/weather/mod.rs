use crate::context::ContextConfig;
use anyhow::Context;
use chrono::Weekday::{Fri, Mon, Sat, Sun};
use chrono::{DateTime, Datelike, Utc};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::ops::Add;
use std::str::FromStr;
use tracing::info;
use url::Url;

const HOUR_COUNT: usize = 5;
const DAY_COUNT: usize = 4;
const TODAY_OFFSET: usize = 0;
const TOMORROW_OFFSET: usize = 24;
const NEXT_WEEK_OFFSET: usize = 24 * 7;
const HOURS: HourIndexes<HOUR_COUNT> = HourIndexes([7, 10, 12, 15, 18]);

#[derive(Debug, Deserialize)]
pub struct AppWeatherConfig {
    pub latitude: String,
    pub longitude: String,
    pub timezone: String,
}

impl ContextConfig for AppWeatherConfig {
    fn name() -> &'static str {
        "weather"
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct WeatherContext {
    pub current: HourWeather,
    pub time: String,
    pub days: [DayWeather; DAY_COUNT],
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct DayWeather {
    pub title: String,
    pub hours: [Option<HourWeather>; HOUR_COUNT],
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq)]
pub struct HourWeather {
    pub icon: WeatherIcon,
    pub temperature: f64,
    pub hour: usize,
    pub precipitation_probability: f64,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq)]
pub enum WeatherIcon {
    #[serde(rename = "sunny")]
    Sunny,
    #[serde(rename = "partly_cloudy_day")]
    PartlyCloudyDay,
    #[serde(rename = "cloud")]
    Cloud,
    #[serde(rename = "cloudy")]
    Cloudy,
    #[serde(rename = "foggy")]
    Foggy,
    #[serde(rename = "rainy_light")]
    RainyLight,
    #[serde(rename = "rainy")]
    Rainy,
    #[serde(rename = "rainy_heavy")]
    RainyHeavy,
    #[serde(rename = "ac_unit")]
    AcUnit,
    #[serde(rename = "severe_cold")]
    SevereCold,
    #[serde(rename = "weather_snowy")]
    WeatherSnowy,
    #[serde(rename = "snowing")]
    Snowing,
    #[serde(rename = "snowing_heavy")]
    SnowingHeavy,
    #[serde(rename = "grain")]
    Grain,
    #[serde(rename = "thunderstorm")]
    Thunderstorm,
    #[serde(rename = "help")]
    Help,
}

pub async fn create_weather_context(config: &AppWeatherConfig) -> anyhow::Result<WeatherContext> {
    info!(
        "Getting weather data for location, with config {:?}",
        config
    );
    let result = get_weather(config).await?;
    let tz: Tz = config.timezone.parse()?;
    let now = Utc::now().with_timezone(&tz);
    parse_weather_data(now, &result)
}

type HourIndex = usize;
struct HourIndexes<const N: usize>([HourIndex; N]);
impl<const N: usize> Add for HourIndexes<N> {
    type Output = HourIndexes<N>;
    fn add(self, HourIndexes(other): HourIndexes<N>) -> HourIndexes<N> {
        let mut result = [0; N];
        for i in 0..N {
            result[i] = self.0[i] + other[i];
        }
        HourIndexes(result)
    }
}

impl HourIndexes<HOUR_COUNT> {
    fn new(offset: usize) -> Self {
        HOURS + HourIndexes([offset; HOUR_COUNT])
    }
}

impl HourWeather {
    fn extract<T>(
        data: &Value,
        hour_index: HourIndex,
        field: &'static str,
        mapper: impl Fn(&Value) -> Option<T>,
    ) -> anyhow::Result<T> {
        let value = &data["hourly"][field][hour_index];
        Ok(mapper(value).context(format!(
            "field error hourly.{}[{}]: {:?}",
            field, hour_index, value
        ))?)
    }

    fn get<T>(
        data: &Value,
        field: &'static str,
        mapper: impl Fn(&Value) -> Option<T>,
    ) -> anyhow::Result<T> {
        let value = &data["current"][field];
        Ok(mapper(value).context(format!("field error current.[{}]: {:?}", field, value))?)
    }

    fn current(data: &Value) -> anyhow::Result<HourWeather> {
        Ok(HourWeather {
            icon: {
                let weather_code = Self::get(data, "weather_code", |v| v.as_u64())?;
                weather_code_to_icon(weather_code)
            },
            temperature: Self::get(data, "temperature_2m", |v| v.as_f64())?,
            hour: 0,
            precipitation_probability: 0.0,
        })
    }

    fn from(data: &Value, hour_index: HourIndex) -> anyhow::Result<HourWeather> {
        Ok(HourWeather {
            icon: {
                let weather_code = Self::extract(data, hour_index, "weather_code", |v| v.as_u64())?;
                weather_code_to_icon(weather_code)
            },
            temperature: Self::extract(data, hour_index, "temperature_2m", |v| v.as_f64())?,
            hour: hour_index % 24,
            precipitation_probability: Self::extract(
                data,
                hour_index,
                "precipitation_probability",
                |v| v.as_f64(),
            )?,
        })
    }
}

impl DayWeather {
    fn from(
        value: &Value,
        title: &str,
        HourIndexes(hour_indexes): HourIndexes<HOUR_COUNT>,
    ) -> anyhow::Result<DayWeather> {
        let mut hours: [Option<HourWeather>; HOUR_COUNT] = [None; HOUR_COUNT];
        for i in 0..HOUR_COUNT {
            hours[i] = Some(HourWeather::from(value, hour_indexes[i])?);
        }
        Ok(DayWeather {
            title: title.to_string(),
            hours,
        })
    }
}

fn weather_code_to_icon(code: u64) -> WeatherIcon {
    match code {
        0 => WeatherIcon::Sunny,
        1 | 2 => WeatherIcon::PartlyCloudyDay,
        3 => WeatherIcon::Cloudy,
        45 | 48 => WeatherIcon::Foggy,
        51 | 61 | 80 => WeatherIcon::RainyLight,
        53 | 63 | 81 => WeatherIcon::Rainy,
        55 | 65 | 82 => WeatherIcon::RainyHeavy,
        56 | 66 => WeatherIcon::AcUnit,
        57 | 67 => WeatherIcon::SevereCold,
        71 | 85 => WeatherIcon::WeatherSnowy,
        73 => WeatherIcon::Snowing,
        75 | 86 => WeatherIcon::SnowingHeavy,
        77 => WeatherIcon::Grain,
        95 | 96 | 99 => WeatherIcon::Thunderstorm,
        _ => WeatherIcon::Help,
    }
}

async fn get_weather(config: &AppWeatherConfig) -> anyhow::Result<Value> {
    let mut url = Url::from_str("https://api.open-meteo.com")?;
    url.set_path("/v1/forecast");
    url.query_pairs_mut()
        .append_pair("latitude", &config.latitude)
        .append_pair("longitude", &config.longitude)
        .append_pair(
            "hourly",
            "temperature_2m,rain,precipitation_probability,weather_code",
        )
        .append_pair(
            "current",
            "temperature_2m,rain,precipitation_probability,weather_code",
        )
        .append_pair("timezone", &config.timezone)
        .append_pair("wind_speed_unit", "mph")
        .append_pair("temperature_unit", "fahrenheit")
        .append_pair("forecast_days", "14")
        .append_pair("precipitation_unit", "inch");

    let result = reqwest::get(url).await?.json().await?;
    Ok(result)
}

fn parse_weather_data(now: DateTime<Tz>, data: &Value) -> anyhow::Result<WeatherContext> {
    let weekday = now.weekday();
    let (third_offset, fourth_offset) = {
        let (third_offset, fourth_offset) = match weekday {
            Fri => (Sun.days_since(weekday), Mon.days_since(weekday)),
            Sat | Sun => (
                NEXT_WEEK_OFFSET as u32 + Sat.days_since(weekday),
                NEXT_WEEK_OFFSET as u32 + Sun.days_since(weekday),
            ),
            _ => (Sat.days_since(weekday), Sun.days_since(weekday)),
        };
        (third_offset as usize, fourth_offset as usize)
    };
    let (third_title, fourth_title) = match weekday {
        Fri => ("Sun", "Mon"),
        Sat | Sun => ("Next Sat", "Next Sun"),
        _ => ("Sat", "Sun"),
    };

    Ok(WeatherContext {
        current: HourWeather::current(data)?,
        time: now.format("%b-%e %l:%M%P").to_string(),
        days: [
            DayWeather::from(data, "Today", HourIndexes::new(TODAY_OFFSET))?,
            DayWeather::from(data, "Tomorrow", HourIndexes::new(TOMORROW_OFFSET))?,
            DayWeather::from(data, third_title, HourIndexes::new(third_offset))?,
            DayWeather::from(data, fourth_title, HourIndexes::new(fourth_offset))?,
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::weather::WeatherIcon::Sunny;
    use chrono_tz::Etc::UTC;

    #[test]
    fn it_should_parse_sample_json() {
        let json = include_str!("sample_forecast.json");
        let result = serde_json::from_str::<Value>(json).unwrap();
        let now = Utc::now().with_timezone(&UTC);
        let result = parse_weather_data(now, &result).unwrap();
        assert!(serde_json::to_string_pretty(&result).is_ok())
    }

    #[test]
    fn it_should_convert_weather_code_to_icon() {
        assert_eq!(weather_code_to_icon(0), Sunny);
    }

    #[ignore = "requires internet connection"]
    #[tokio::test]
    async fn it_should_get_weather() {
        let config = AppWeatherConfig {
            latitude: "45.5234".to_string(),
            longitude: "-122.6762".to_string(),
            timezone: "America/Los_Angeles".to_string(),
        };
        let result = get_weather(&config).await;
        assert!(result.is_ok());
        let result = result.unwrap();
        println!("{:?}", result);
    }
}
