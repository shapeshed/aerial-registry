#!/usr/bin/env python3
"""Regenerate docs/coverage.md from a built registry.

Usage: build a registry first (registry.json.gz in the repo root), then

    python3 scripts/coverage.py

A country is checked when at least one direct (non-curated) provider serves
it. Curated-only coverage is shown alongside so gaps with existing listener
interest stand out.
"""

import gzip
import json
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
REGISTRY = ROOT / "registry.json.gz"
OUT = ROOT / "docs" / "coverage.md"

# ISO 3166-1 alpha-2 → (name, region). UN members plus Kosovo, Palestine,
# Taiwan and Vatican City. Dependent territories with stations are listed in
# their own section, driven by whatever codes appear in the registry.
COUNTRIES = {
    # Europe
    "AL": ("Albania", "Europe"), "AD": ("Andorra", "Europe"), "AT": ("Austria", "Europe"),
    "BY": ("Belarus", "Europe"), "BE": ("Belgium", "Europe"), "BA": ("Bosnia and Herzegovina", "Europe"),
    "BG": ("Bulgaria", "Europe"), "HR": ("Croatia", "Europe"), "CY": ("Cyprus", "Europe"),
    "CZ": ("Czechia", "Europe"), "DK": ("Denmark", "Europe"), "EE": ("Estonia", "Europe"),
    "FI": ("Finland", "Europe"), "FR": ("France", "Europe"), "DE": ("Germany", "Europe"),
    "GR": ("Greece", "Europe"), "HU": ("Hungary", "Europe"), "IS": ("Iceland", "Europe"),
    "IE": ("Ireland", "Europe"), "IT": ("Italy", "Europe"), "XK": ("Kosovo", "Europe"),
    "LV": ("Latvia", "Europe"), "LI": ("Liechtenstein", "Europe"), "LT": ("Lithuania", "Europe"),
    "LU": ("Luxembourg", "Europe"), "MT": ("Malta", "Europe"), "MD": ("Moldova", "Europe"),
    "MC": ("Monaco", "Europe"), "ME": ("Montenegro", "Europe"), "NL": ("Netherlands", "Europe"),
    "MK": ("North Macedonia", "Europe"), "NO": ("Norway", "Europe"), "PL": ("Poland", "Europe"),
    "PT": ("Portugal", "Europe"), "RO": ("Romania", "Europe"), "RU": ("Russia", "Europe"),
    "SM": ("San Marino", "Europe"), "RS": ("Serbia", "Europe"), "SK": ("Slovakia", "Europe"),
    "SI": ("Slovenia", "Europe"), "ES": ("Spain", "Europe"), "SE": ("Sweden", "Europe"),
    "CH": ("Switzerland", "Europe"), "UA": ("Ukraine", "Europe"), "GB": ("United Kingdom", "Europe"),
    "VA": ("Vatican City", "Europe"),
    # Africa
    "DZ": ("Algeria", "Africa"), "AO": ("Angola", "Africa"), "BJ": ("Benin", "Africa"),
    "BW": ("Botswana", "Africa"), "BF": ("Burkina Faso", "Africa"), "BI": ("Burundi", "Africa"),
    "CV": ("Cabo Verde", "Africa"), "CM": ("Cameroon", "Africa"), "CF": ("Central African Republic", "Africa"),
    "TD": ("Chad", "Africa"), "KM": ("Comoros", "Africa"), "CG": ("Congo", "Africa"),
    "CD": ("DR Congo", "Africa"), "CI": ("Côte d'Ivoire", "Africa"), "DJ": ("Djibouti", "Africa"),
    "EG": ("Egypt", "Africa"), "GQ": ("Equatorial Guinea", "Africa"), "ER": ("Eritrea", "Africa"),
    "SZ": ("Eswatini", "Africa"), "ET": ("Ethiopia", "Africa"), "GA": ("Gabon", "Africa"),
    "GM": ("Gambia", "Africa"), "GH": ("Ghana", "Africa"), "GN": ("Guinea", "Africa"),
    "GW": ("Guinea-Bissau", "Africa"), "KE": ("Kenya", "Africa"), "LS": ("Lesotho", "Africa"),
    "LR": ("Liberia", "Africa"), "LY": ("Libya", "Africa"), "MG": ("Madagascar", "Africa"),
    "MW": ("Malawi", "Africa"), "ML": ("Mali", "Africa"), "MR": ("Mauritania", "Africa"),
    "MU": ("Mauritius", "Africa"), "MA": ("Morocco", "Africa"), "MZ": ("Mozambique", "Africa"),
    "NA": ("Namibia", "Africa"), "NE": ("Niger", "Africa"), "NG": ("Nigeria", "Africa"),
    "RW": ("Rwanda", "Africa"), "ST": ("São Tomé and Príncipe", "Africa"), "SN": ("Senegal", "Africa"),
    "SC": ("Seychelles", "Africa"), "SL": ("Sierra Leone", "Africa"), "SO": ("Somalia", "Africa"),
    "ZA": ("South Africa", "Africa"), "SS": ("South Sudan", "Africa"), "SD": ("Sudan", "Africa"),
    "TZ": ("Tanzania", "Africa"), "TG": ("Togo", "Africa"), "TN": ("Tunisia", "Africa"),
    "UG": ("Uganda", "Africa"), "ZM": ("Zambia", "Africa"), "ZW": ("Zimbabwe", "Africa"),
    # Asia
    "AF": ("Afghanistan", "Asia"), "AM": ("Armenia", "Asia"), "AZ": ("Azerbaijan", "Asia"),
    "BH": ("Bahrain", "Asia"), "BD": ("Bangladesh", "Asia"), "BT": ("Bhutan", "Asia"),
    "BN": ("Brunei", "Asia"), "KH": ("Cambodia", "Asia"), "CN": ("China", "Asia"),
    "GE": ("Georgia", "Asia"), "IN": ("India", "Asia"), "ID": ("Indonesia", "Asia"),
    "IR": ("Iran", "Asia"), "IQ": ("Iraq", "Asia"), "IL": ("Israel", "Asia"),
    "JP": ("Japan", "Asia"), "JO": ("Jordan", "Asia"), "KZ": ("Kazakhstan", "Asia"),
    "KW": ("Kuwait", "Asia"), "KG": ("Kyrgyzstan", "Asia"), "LA": ("Laos", "Asia"),
    "LB": ("Lebanon", "Asia"), "MY": ("Malaysia", "Asia"), "MV": ("Maldives", "Asia"),
    "MN": ("Mongolia", "Asia"), "MM": ("Myanmar", "Asia"), "NP": ("Nepal", "Asia"),
    "KP": ("North Korea", "Asia"), "OM": ("Oman", "Asia"), "PK": ("Pakistan", "Asia"),
    "PS": ("Palestine", "Asia"), "PH": ("Philippines", "Asia"), "QA": ("Qatar", "Asia"),
    "SA": ("Saudi Arabia", "Asia"), "SG": ("Singapore", "Asia"), "KR": ("South Korea", "Asia"),
    "LK": ("Sri Lanka", "Asia"), "SY": ("Syria", "Asia"), "TW": ("Taiwan", "Asia"),
    "TJ": ("Tajikistan", "Asia"), "TH": ("Thailand", "Asia"), "TL": ("Timor-Leste", "Asia"),
    "TR": ("Türkiye", "Asia"), "TM": ("Turkmenistan", "Asia"), "AE": ("United Arab Emirates", "Asia"),
    "UZ": ("Uzbekistan", "Asia"), "VN": ("Vietnam", "Asia"), "YE": ("Yemen", "Asia"),
    # North America (incl. Central America and the Caribbean)
    "AG": ("Antigua and Barbuda", "North America"), "BS": ("Bahamas", "North America"),
    "BB": ("Barbados", "North America"), "BZ": ("Belize", "North America"),
    "CA": ("Canada", "North America"), "CR": ("Costa Rica", "North America"),
    "CU": ("Cuba", "North America"), "DM": ("Dominica", "North America"),
    "DO": ("Dominican Republic", "North America"), "SV": ("El Salvador", "North America"),
    "GD": ("Grenada", "North America"), "GT": ("Guatemala", "North America"),
    "HT": ("Haiti", "North America"), "HN": ("Honduras", "North America"),
    "JM": ("Jamaica", "North America"), "MX": ("Mexico", "North America"),
    "NI": ("Nicaragua", "North America"), "PA": ("Panama", "North America"),
    "KN": ("Saint Kitts and Nevis", "North America"), "LC": ("Saint Lucia", "North America"),
    "VC": ("Saint Vincent and the Grenadines", "North America"), "TT": ("Trinidad and Tobago", "North America"),
    "US": ("United States", "North America"),
    # South America
    "AR": ("Argentina", "South America"), "BO": ("Bolivia", "South America"),
    "BR": ("Brazil", "South America"), "CL": ("Chile", "South America"),
    "CO": ("Colombia", "South America"), "EC": ("Ecuador", "South America"),
    "GY": ("Guyana", "South America"), "PY": ("Paraguay", "South America"),
    "PE": ("Peru", "South America"), "SR": ("Suriname", "South America"),
    "UY": ("Uruguay", "South America"), "VE": ("Venezuela", "South America"),
    # Oceania
    "AU": ("Australia", "Oceania"), "FJ": ("Fiji", "Oceania"), "KI": ("Kiribati", "Oceania"),
    "MH": ("Marshall Islands", "Oceania"), "FM": ("Micronesia", "Oceania"), "NR": ("Nauru", "Oceania"),
    "NZ": ("New Zealand", "Oceania"), "PW": ("Palau", "Oceania"), "PG": ("Papua New Guinea", "Oceania"),
    "WS": ("Samoa", "Oceania"), "SB": ("Solomon Islands", "Oceania"), "TO": ("Tonga", "Oceania"),
    "TV": ("Tuvalu", "Oceania"), "VU": ("Vanuatu", "Oceania"),
}

REGIONS = ["Europe", "Africa", "Asia", "North America", "South America", "Oceania"]


def load_coverage():
    with gzip.open(REGISTRY) as f:
        stations = json.load(f)
    by_cc = defaultdict(lambda: defaultdict(int))
    for s in stations:
        cc = (s.get("country_code") or "??").upper()
        by_cc[cc][s["provider"]] += 1
    return by_cc


def line(name, providers):
    direct = {p: n for p, n in providers.items() if p != "curated"}
    curated = providers.get("curated", 0)
    if direct:
        provs = ", ".join(f"{p} ({n})" for p, n in sorted(direct.items()))
        extra = f" · {curated} curated" if curated else ""
        return f"- [x] **{name}** — {provs}{extra}"
    if curated:
        return f"- [ ] {name} *({curated} curated)*"
    return f"- [ ] {name}"


def main():
    by_cc = load_coverage()
    checked = sum(1 for cc in COUNTRIES if any(p != "curated" for p in by_cc.get(cc, {})))
    any_station = sum(1 for cc in COUNTRIES if cc in by_cc)

    out = [
        "# Country coverage",
        "",
        "Generated by `python3 scripts/coverage.py` from a built registry — do not",
        "edit by hand. A checked country has at least one direct (trusted) provider;",
        "*(n curated)* marks countries served only by curated/aggregator stations —",
        "existing listener interest with no direct integration yet.",
        "",
        f"**{checked}** of {len(COUNTRIES)} countries have a direct provider; "
        f"**{any_station}** have at least one station.",
        "",
    ]
    for region in REGIONS:
        rows = [(name, cc) for cc, (name, r) in COUNTRIES.items() if r == region]
        rows.sort()
        done = sum(1 for _, cc in rows if any(p != "curated" for p in by_cc.get(cc, {})))
        out.append(f"## {region} ({done}/{len(rows)})")
        out.append("")
        for name, cc in rows:
            out.append(line(name, by_cc.get(cc, {})))
        out.append("")

    territories = sorted(cc for cc in by_cc if cc not in COUNTRIES and cc != "??")
    if territories:
        out.append("## Territories with stations")
        out.append("")
        for cc in territories:
            provs = dict(by_cc[cc])
            total = sum(provs.values())
            out.append(f"- {cc} — {total} station{'s' if total != 1 else ''}")
        out.append("")

    OUT.write_text("\n".join(out))
    print(f"wrote {OUT} — {checked}/{len(COUNTRIES)} direct, {any_station} with stations")


if __name__ == "__main__":
    main()
