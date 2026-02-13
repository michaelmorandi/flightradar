"""
Airline Service

Loads the Mictronics operators.json database and provides airline lookup
and search functionality by ICAO 3-letter designator.
"""

import json
import logging
from pathlib import Path
from typing import Optional

logger = logging.getLogger(__name__)


class AirlineInfo:
    __slots__ = ('icao_code', 'name', 'country', 'callsign')

    def __init__(self, icao_code: str, name: str, country: str, callsign: str):
        self.icao_code = icao_code
        self.name = name
        self.country = country
        self.callsign = callsign

    def to_dict(self):
        return {
            'icao_code': self.icao_code,
            'name': self.name,
            'country': self.country,
            'callsign': self.callsign,
        }


class AirlineService:
    """In-memory airline lookup service backed by operators.json."""

    def __init__(self, data_folder: str):
        self._airlines: dict[str, AirlineInfo] = {}
        self._search_index: list[tuple[str, str]] = []  # [(lowercase_name, icao_code), ...]
        self._load(data_folder)

    def _load(self, data_folder: str):
        operators_path = Path(data_folder) / 'operators.json'
        if not operators_path.exists():
            logger.warning(f"operators.json not found at {operators_path}")
            return

        with open(operators_path, 'r', encoding='utf-8') as f:
            data = json.load(f)

        for icao_code, values in data.items():
            if not isinstance(values, list) or len(values) < 3:
                continue
            airline = AirlineInfo(
                icao_code=icao_code,
                name=values[0],
                country=values[1],
                callsign=values[2],
            )
            self._airlines[icao_code] = airline
            self._search_index.append((values[0].lower(), icao_code))

        # Sort search index by name for consistent results
        self._search_index.sort(key=lambda x: x[0])
        logger.info(f"Loaded {len(self._airlines)} airline operators from {operators_path}")

    def get(self, icao_code: str) -> Optional[AirlineInfo]:
        """Look up airline by ICAO 3-letter code."""
        return self._airlines.get(icao_code.upper()) if icao_code else None

    def search(self, query: str, limit: int = 50) -> list[AirlineInfo]:
        """Search airlines by name or ICAO code (case-insensitive prefix match)."""
        if not query:
            return []

        q = query.strip().upper()
        results = []

        # Exact ICAO code match first
        if len(q) <= 3 and q in self._airlines:
            results.append(self._airlines[q])

        q_lower = q.lower()

        # Name prefix search
        for name_lower, icao_code in self._search_index:
            if len(results) >= limit:
                break
            airline = self._airlines[icao_code]
            if airline in results:
                continue
            if name_lower.startswith(q_lower) or icao_code.startswith(q):
                results.append(airline)

        # Substring search if few results
        if len(results) < limit:
            for name_lower, icao_code in self._search_index:
                if len(results) >= limit:
                    break
                airline = self._airlines[icao_code]
                if airline in results:
                    continue
                if q_lower in name_lower:
                    results.append(airline)

        return results[:limit]

    @property
    def count(self) -> int:
        return len(self._airlines)
