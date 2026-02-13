"""
Callsign Parsing Utility

Extracts ICAO 3-letter airline designators from ADS-B callsigns.

Commercial flights use the format: 3-letter ICAO airline code + flight number
  e.g. "AFR990" -> "AFR" (Air France), "BAW238" -> "BAW" (British Airways)

General aviation aircraft transmit their registration as callsign
  e.g. "N172SP", "G-ABCD", "D-EABC" -> not an airline callsign
"""

import re

# Patterns for GA registrations (country prefix + number/letter)
_GA_PATTERNS = re.compile(
    r'^[A-Z]{1,2}-'   # Country prefix with dash (G-ABCD, D-EABC, HB-JCS, etc.)
    r'|^N\d'          # US N-number (N172SP)
    r'|^F-'           # France
    r'|^VH-'          # Australia
    r'|^ZK-'          # New Zealand
    r'|^JA\d'         # Japan (JA followed by digits)
)

# Privacy/relay callsign prefixes (not real airlines)
_PRIVACY_PREFIXES = {'DCM', 'FFL', 'FWR', 'XAA'}


def extract_airline_icao(callsign: str) -> str | None:
    """
    Extract ICAO 3-letter airline designator from an ADS-B callsign.

    Returns the 3-letter code (uppercase) if the callsign matches a commercial
    airline pattern, or None for GA/private/military/unrecognized callsigns.
    """
    if not callsign or len(callsign) < 4:
        return None

    cs = callsign.strip().upper()
    if not cs:
        return None

    # Skip GA registrations
    if _GA_PATTERNS.match(cs):
        return None

    prefix = cs[:3]

    # Skip privacy/relay prefixes
    if prefix in _PRIVACY_PREFIXES:
        return None

    # Commercial pattern: first 3 chars are alpha, remainder has at least one digit
    if prefix.isalpha() and any(c.isdigit() for c in cs[3:]):
        return prefix

    return None
