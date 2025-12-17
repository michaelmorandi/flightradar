class PositionReport:
    # Aircraft category enum values from adsb.proto
    CATEGORY_MAP = {
        'AIRCRAFT_CATEGORY_UNKNOWN': 0,
        'AIRCRAFT_CATEGORY_NO_INFO': 1,
        'AIRCRAFT_CATEGORY_LIGHT': 2,
        'AIRCRAFT_CATEGORY_MEDIUM_1': 3,
        'AIRCRAFT_CATEGORY_MEDIUM_2': 4,
        'AIRCRAFT_CATEGORY_HIGH_VORTEX_LARGE': 5,
        'AIRCRAFT_CATEGORY_HEAVY': 6,
        'AIRCRAFT_CATEGORY_HIGH_PERFORMANCE': 7,
        'AIRCRAFT_CATEGORY_ROTORCRAFT': 8,
        'AIRCRAFT_CATEGORY_GLIDER': 9,
        'AIRCRAFT_CATEGORY_LIGHTER_THAN_AIR': 10,
        'AIRCRAFT_CATEGORY_PARACHUTIST': 11,
        'AIRCRAFT_CATEGORY_ULTRALIGHT': 12,
        'AIRCRAFT_CATEGORY_UAV': 13,
        'AIRCRAFT_CATEGORY_SPACE': 14,
        'AIRCRAFT_CATEGORY_SURFACE_EMERGENCY': 15,
        'AIRCRAFT_CATEGORY_SURFACE_SERVICE': 16,
        'AIRCRAFT_CATEGORY_POINT_OBSTACLE': 17,
        'AIRCRAFT_CATEGORY_CLUSTER_OBSTACLE': 18,
        'AIRCRAFT_CATEGORY_LINE_OBSTACLE': 19,
        'AIRCRAFT_CATEGORY_RESERVED': 20,
    }

    def __init__(self, icao24: str, lat, lon, alt, gs=None, track=None, callsign=None, category=None):  # gs = ground speed
        self.icao24 = icao24
        self.lat = lat
        self.lon = lon
        self.alt = alt
        self.gs = gs  # ground speed
        self.track = track
        self.callsign = callsign
        self.category = category

    def __eq__(self, other):

        if not isinstance(other, PositionReport):
            # don't attempt to compare against unrelated types
            return NotImplemented

        return self.icao24 == other.icao24 \
            and self.lat == other.lat \
            and self.lon == other.lon \
            and self.alt == other.alt \
            and self.gs == other.gs \
            and self.track == other.track \
            and self.callsign == other.callsign \
            and self.category == other.category