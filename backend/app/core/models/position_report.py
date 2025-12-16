class PositionReport:
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