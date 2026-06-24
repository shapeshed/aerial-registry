use reqwest::Client;
use tracing::debug;

use crate::station::Station;

const COUNTRY_CODE: &str = "ES";
const COUNTRY_NAME: &str = "Spain";

const RNE_LOGO: &str = "https://graph.facebook.com/radionacionalrne/picture?width=200&height=200";
const RNE_CLASICA_LOGO: &str =
    "https://graph.facebook.com/radioclasicartve/picture?width=200&height=200";
const RNE_3_LOGO: &str = "https://graph.facebook.com/radio3/picture?width=200&height=200";
const RNE_4_LOGO: &str = "https://graph.facebook.com/Radio4RNE/picture?width=200&height=200";
const RNE_5_LOGO: &str =
    "https://pbs.twimg.com/profile_images/1405097207339028480/H7nP_7Ti_200x200.jpg";
const RNE_EXTERIOR_LOGO: &str = "https://upload.wikimedia.org/wikipedia/commons/thumb/8/86/Radio_Exterior_RNE_Spain.svg/320px-Radio_Exterior_RNE_Spain.svg.png";

struct RtveStation {
    provider_id: &'static str,
    name: &'static str,
    stream_url: &'static str,
    logo_url: &'static str,
    tags: &'static [&'static str],
    description: &'static str,
}

struct RneRegionalStation {
    channel: &'static str,
    region_code: &'static str,
    region_name: &'static str,
    homepage: &'static str,
}

const NATIONAL_STATIONS: &[RtveStation] = &[
    RtveStation {
        provider_id: "rtve:rne1",
        name: "Radio Nacional",
        stream_url: "https://rtvelivestream.rtve.es/rtvesec/rne/rne_r1_main.m3u8",
        logo_url: RNE_LOGO,
        tags: &["news", "speech", "public radio"],
        description: "Radio Nacional de España",
    },
    RtveStation {
        provider_id: "rtve:rne2",
        name: "Radio Clásica",
        stream_url: "https://rtvelivestream.rtve.es/rtvesec/rne/rne_r2_main.m3u8",
        logo_url: RNE_CLASICA_LOGO,
        tags: &["classical", "music", "public radio"],
        description: "Classical music from Radio Nacional de España",
    },
    RtveStation {
        provider_id: "rtve:rne3",
        name: "Radio 3",
        stream_url: "https://rtvelivestream.rtve.es/rtvesec/rne/rne_r3_main.m3u8",
        logo_url: RNE_3_LOGO,
        tags: &["music", "culture", "alternative", "public radio"],
        description: "Music and culture from Radio Nacional de España",
    },
    RtveStation {
        provider_id: "rtve:rne4",
        name: "Ràdio 4",
        stream_url: "https://rtvelivestream.rtve.es/rtvesec/rne/rne_r4_main.m3u8",
        logo_url: RNE_4_LOGO,
        tags: &["catalan", "speech", "public radio"],
        description: "RNE radio service in Catalan",
    },
    RtveStation {
        provider_id: "rtve:rne5",
        name: "Radio 5",
        stream_url: "https://rtvelivestream.rtve.es/rtvesec/rne/rne_r5_madrid_main.m3u8",
        logo_url: RNE_5_LOGO,
        tags: &["news", "public radio"],
        description: "24-hour news from Radio Nacional de España",
    },
    RtveStation {
        provider_id: "rtve:rne-exterior",
        name: "Radio Exterior",
        stream_url: "https://rtvelivestream.rtve.es/rtvesec/rne/rne_re_main.m3u8",
        logo_url: RNE_EXTERIOR_LOGO,
        tags: &["international", "news", "public radio"],
        description: "International service from Radio Nacional de España",
    },
];

const RNE1_REGIONS: &[RneRegionalStation] = &[
    RneRegionalStation {
        channel: "rne1",
        region_code: "and",
        region_name: "Andalucía",
        homepage: "https://www.rtve.es/play/audios/programa/rne_and-live/3893442/",
    },
    RneRegionalStation {
        channel: "rne1",
        region_code: "ara",
        region_name: "Aragón",
        homepage: "https://www.rtve.es/play/audios/programa/rne_ara-live/3893524/",
    },
    RneRegionalStation {
        channel: "rne1",
        region_code: "ast",
        region_name: "Asturias",
        homepage: "https://www.rtve.es/play/audios/programa/rne_ast-live/3893526/",
    },
    RneRegionalStation {
        channel: "rne1",
        region_code: "bal",
        region_name: "Islas Baleares",
        homepage: "https://www.rtve.es/play/audios/programa/rne_bal/3893546/",
    },
    RneRegionalStation {
        channel: "rne1",
        region_code: "cat",
        region_name: "Catalunya",
        homepage: "https://www.rtve.es/play/audios/programa/rne_cat-live/3893527/",
    },
    RneRegionalStation {
        channel: "rne1",
        region_code: "ceu",
        region_name: "Ceuta",
        homepage: "https://www.rtve.es/play/audios/programa/radio1-ceuta/4143261/",
    },
    RneRegionalStation {
        channel: "rne1",
        region_code: "clm",
        region_name: "Castilla-La Mancha",
        homepage: "https://www.rtve.es/play/audios/programa/rne_clm-live/3893547/",
    },
    RneRegionalStation {
        channel: "rne1",
        region_code: "cnr",
        region_name: "Canarias",
        homepage: "https://www.rtve.es/play/audios/programa/rne_ten-live/3893530/",
    },
    RneRegionalStation {
        channel: "rne1",
        region_code: "ctb",
        region_name: "Cantabria",
        homepage: "https://www.rtve.es/play/audios/programa/rne_ctb-live/3893531/",
    },
    RneRegionalStation {
        channel: "rne1",
        region_code: "cyl",
        region_name: "Castilla y León",
        homepage: "https://www.rtve.es/play/audios/programa/rne_cyl-live/3893532/",
    },
    RneRegionalStation {
        channel: "rne1",
        region_code: "eus",
        region_name: "País Vasco",
        homepage: "https://www.rtve.es/play/audios/programa/rne_eus-live/3893548/",
    },
    RneRegionalStation {
        channel: "rne1",
        region_code: "ext",
        region_name: "Extremadura",
        homepage: "https://www.rtve.es/play/audios/programa/rne_ext-live/3893533/",
    },
    RneRegionalStation {
        channel: "rne1",
        region_code: "gal",
        region_name: "Galicia",
        homepage: "https://www.rtve.es/play/audios/programa/rne_gal-live/3893549/",
    },
    RneRegionalStation {
        channel: "rne1",
        region_code: "mad",
        region_name: "Madrid",
        homepage: "https://www.rtve.es/play/audios/programa/rne_mad-live/3893534/",
    },
    RneRegionalStation {
        channel: "rne1",
        region_code: "mel",
        region_name: "Melilla",
        homepage: "https://www.rtve.es/play/audios/programa/radio1-melilla/4143282/",
    },
    RneRegionalStation {
        channel: "rne1",
        region_code: "nav",
        region_name: "Navarra",
        homepage: "https://www.rtve.es/play/audios/programa/rne_nav-live/3893551/",
    },
    RneRegionalStation {
        channel: "rne1",
        region_code: "rio",
        region_name: "La Rioja",
        homepage: "https://www.rtve.es/play/audios/programa/rne_rio-live/3893536/",
    },
    RneRegionalStation {
        channel: "rne1",
        region_code: "val",
        region_name: "Comunidad Valenciana",
        homepage: "https://www.rtve.es/play/audios/programa/rne_val-live/3893537/",
    },
];

const RNE5_REGIONS: &[RneRegionalStation] = &[
    RneRegionalStation {
        channel: "rne5",
        region_code: "sev",
        region_name: "Andalucía",
        homepage: "https://www.rtve.es/play/audios/programa/rne5_and/3894738/",
    },
    RneRegionalStation {
        channel: "rne5",
        region_code: "zgz",
        region_name: "Aragón",
        homepage: "https://www.rtve.es/play/audios/programa/rne5_ara/3894741/",
    },
    RneRegionalStation {
        channel: "rne5",
        region_code: "ast",
        region_name: "Asturias",
        homepage: "https://www.rtve.es/play/audios/programa/rne5_ast/3894742/",
    },
    RneRegionalStation {
        channel: "rne5",
        region_code: "pmi",
        region_name: "Islas Baleares",
        homepage: "https://www.rtve.es/play/audios/programa/rne5_bal/3894743/",
    },
    RneRegionalStation {
        channel: "rne5",
        region_code: "lpa",
        region_name: "Canarias - Las Palmas",
        homepage: "https://www.rtve.es/play/audios/programa/rne_ten-live/3893530/",
    },
    RneRegionalStation {
        channel: "rne5",
        region_code: "ten",
        region_name: "Canarias - Santa Cruz de Tenerife",
        homepage: "https://www.rtve.es/play/audios/programa/rne5_cnr-live/3894726/",
    },
    RneRegionalStation {
        channel: "rne5",
        region_code: "ctb",
        region_name: "Cantabria",
        homepage: "https://www.rtve.es/play/audios/programa/rne5_ctb-live/3894780/",
    },
    RneRegionalStation {
        channel: "rne5",
        region_code: "tol",
        region_name: "Castilla-La Mancha",
        homepage: "https://www.rtve.es/play/audios/programa/rne5_clm-live/3894725/",
    },
    RneRegionalStation {
        channel: "rne5",
        region_code: "vll",
        region_name: "Castilla y León",
        homepage: "https://www.rtve.es/play/audios/programa/rne5_cyl-live/3894781/",
    },
    RneRegionalStation {
        channel: "rne5",
        region_code: "bcn",
        region_name: "Catalunya",
        homepage: "https://www.rtve.es/play/audios/programa/rne5_cat/3894724/",
    },
    RneRegionalStation {
        channel: "rne5",
        region_code: "ceu",
        region_name: "Ceuta",
        homepage: "https://www.rtve.es/play/audios/programa/radio5-ceuta/4143281/",
    },
    RneRegionalStation {
        channel: "rne5",
        region_code: "mad",
        region_name: "Madrid",
        homepage: "https://www.rtve.es/play/audios/programa/rne5_mad-live/3894730/",
    },
    RneRegionalStation {
        channel: "rne5",
        region_code: "nav",
        region_name: "Navarra",
        homepage: "https://www.rtve.es/play/audios/programa/rne5_nav-live/3894787/",
    },
    RneRegionalStation {
        channel: "rne5",
        region_code: "vlc",
        region_name: "Comunidad Valenciana",
        homepage: "https://www.rtve.es/play/audios/programa/rne5_val-live/3894731/",
    },
    RneRegionalStation {
        channel: "rne5",
        region_code: "cce",
        region_name: "Extremadura",
        homepage: "https://www.rtve.es/play/audios/programa/rne5_ext-live/3894783/",
    },
    RneRegionalStation {
        channel: "rne5",
        region_code: "lcg",
        region_name: "Galicia",
        homepage: "https://www.rtve.es/play/audios/programa/rne5_gal-live/3894729/",
    },
    RneRegionalStation {
        channel: "rne5",
        region_code: "rio",
        region_name: "La Rioja",
        homepage: "https://www.rtve.es/play/audios/programa/rne5_rio-live/3894788/",
    },
    RneRegionalStation {
        channel: "rne5",
        region_code: "mel",
        region_name: "Melilla",
        homepage: "https://www.rtve.es/play/audios/programa/radio5-melilla/4143283/",
    },
    RneRegionalStation {
        channel: "rne5",
        region_code: "vit",
        region_name: "País Vasco",
        homepage: "https://www.rtve.es/play/audios/programa/rne5_eus-live/3894782/",
    },
];

fn regional_stream_url(channel: &str, region_code: &str) -> String {
    format!(
        "https://rnelivestream.rtve.es/{}/{}/128/seglist.m3u8",
        channel, region_code
    )
}

fn regional_name(channel: &str, region_name: &str) -> String {
    match channel {
        "rne1" => format!("Radio Nacional {region_name}"),
        "rne5" => format!("Radio 5 {region_name}"),
        _ => format!("RNE {region_name}"),
    }
}

fn regional_logo(channel: &str) -> &'static str {
    match channel {
        "rne5" => RNE_5_LOGO,
        _ => RNE_LOGO,
    }
}

fn regional_tags(channel: &str) -> Vec<String> {
    match channel {
        "rne5" => vec!["news".into(), "regional".into(), "public radio".into()],
        _ => vec!["speech".into(), "regional".into(), "public radio".into()],
    }
}

pub async fn discover(_client: &Client) -> Vec<Station> {
    let mut stations = Vec::new();

    for s in NATIONAL_STATIONS {
        debug!(
            provider = "rtve",
            name = s.name,
            stream_url = s.stream_url,
            "Discovered station"
        );

        stations.push(Station {
            name: s.name.to_string(),
            stream_url: s.stream_url.to_string(),
            logo_url: Some(s.logo_url.to_string()),
            country: Some(COUNTRY_NAME.to_string()),
            country_code: Some(COUNTRY_CODE.to_string()),
            tags: s.tags.iter().map(|t| t.to_string()).collect(),
            description: Some(s.description.to_string()),
            provider: "rtve".into(),
            provider_id: Some(s.provider_id.to_string()),
            trusted: true,
        });
    }

    for s in RNE1_REGIONS.iter().chain(RNE5_REGIONS.iter()) {
        let name = regional_name(s.channel, s.region_name);
        let stream_url = regional_stream_url(s.channel, s.region_code);
        let provider_id = format!("rtve:{}:{}", s.channel, s.region_code);

        debug!(
            provider = "rtve",
            %name,
            %stream_url,
            homepage = s.homepage,
            "Discovered regional station"
        );

        stations.push(Station {
            name,
            stream_url,
            logo_url: Some(regional_logo(s.channel).to_string()),
            country: Some(COUNTRY_NAME.to_string()),
            country_code: Some(COUNTRY_CODE.to_string()),
            tags: regional_tags(s.channel),
            description: Some(format!(
                "Regional {} service for {}",
                s.channel, s.region_name
            )),
            provider: "rtve".into(),
            provider_id: Some(provider_id),
            trusted: true,
        });
    }

    tracing::info!(
        provider = "rtve",
        count = stations.len(),
        "Discovery complete"
    );

    stations
}
