use serde::Deserialize;

#[derive(Debug, Clone, Default)]
pub struct PublicInfo {
    pub ip: String,
    pub city: Option<String>,
    pub region: Option<String>,
    pub country: Option<String>,
    pub org: Option<String>,
    pub timezone: Option<String>,
}

#[derive(Debug, Deserialize)]
struct IpApiResponse {
    ip: String,
    #[serde(default)]
    city: Option<String>,
    #[serde(default)]
    region: Option<String>,
    #[serde(default)]
    country_name: Option<String>,
    #[serde(default)]
    org: Option<String>,
    #[serde(default)]
    timezone: Option<String>,
}

pub async fn fetch() -> Result<PublicInfo, String> {
    let client = crate::util::http_client()?;

    match client
        .get("https://ipapi.co/json/")
        .header("Accept", "application/json")
        .send()
        .await
    {
        Ok(response) if response.status().is_success() => {
            match response.json::<IpApiResponse>().await {
                Ok(data) if !data.ip.trim().is_empty() && data.ip != "None" => {
                    return Ok(PublicInfo {
                        ip: data.ip,
                        city: data.city,
                        region: data.region,
                        country: data.country_name,
                        org: data.org,
                        timezone: data.timezone,
                    });
                }
                _ => {}
            }
        }
        _ => {}
    }

    // Fallback: only the public address, without geolocation.
    let response = client
        .get("https://api.ipify.org")
        .send()
        .await
        .map_err(|error| format!("Unable to reach the Internet: {error}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "The public IP service responded {}",
            response.status()
        ));
    }

    let ip = response
        .text()
        .await
        .map_err(|error| format!("Unreadable public IP response: {error}"))?
        .trim()
        .to_owned();

    if ip.is_empty() {
        return Err("The public IP service returned no address".to_owned());
    }

    Ok(PublicInfo {
        ip,
        ..Default::default()
    })
}
