import json
import string
from os import path


class ModesUtil:
    """
    Utility class for Mode-S / ICAO24 address operations.

    Military ranges are loaded from mil_ranges.json, sourced from:
    https://github.com/wiedehopf/tar1090-db
    """

    def __init__(self, folder):
        self.ranges = []

        file_name = path.join(folder, 'mil_ranges.json')

        with open(file_name, 'r') as f:
            data = json.load(f)
            # Convert hex string ranges to integer tuples (start, end)
            for range_pair in data.get('military', []):
                start = int(range_pair[0], 16)
                end = int(range_pair[1], 16)
                self.ranges.append((start, end))

    @staticmethod
    def is_icao24_addr(icao24: str):
        return len(icao24) == 6 and all(c in string.hexdigits for c in icao24)

    def is_military(self, icao24: str) -> bool:
        """
        Returns true if the ICAO24 address falls within a known military range.

        Args:
            icao24: 6-character hex string (e.g., "4B7123")

        Returns:
            True if the address is in a military range
        """
        icao_nr = int(icao24, 16)

        for start, end in self.ranges:
            if start <= icao_nr <= end:
                return True
        return False

    @staticmethod
    def is_swiss_mil(icao: int) -> bool:
        """Returns true if the ICAO code (as int) is in the Swiss military range."""
        return 0x4B7000 <= icao <= 0x4B7FFF

    @staticmethod
    def is_swiss(icaohex: str) -> bool:
        """Returns true if the ICAO hex address is Swiss."""
        if icaohex and icaohex[0:2].upper() == "4B":
            third = int(icaohex[2], 16)
            if 0 <= third <= 8:
                return True
        return False
