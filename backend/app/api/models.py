from datetime import datetime
from pydantic import BaseModel, Field
from typing import Optional, List

def to_datestring(obj: datetime) -> str:
    if obj.tzinfo:
        # eg: '2015-09-25T23:14:42.588601+00:00'
        return obj.isoformat('T')
    else:
        # No timezone present - assume UTC.
        # eg: '2015-09-25T23:14:42.588601Z'
        return obj.isoformat('T') + 'Z'

class FlightDto(BaseModel):
    id: str
    icao24: str
    cls: Optional[str] = None
    lstCntct: str
    firstCntct: str
    positionCount: Optional[int] = Field(None, alias="position_count")

    class Config:
        arbitrary_types_allowed = True
        populate_by_name = True

class PaginatedFlightsResponse(BaseModel):
    flights: List[FlightDto]
    total: int
    page: int
    page_size: int = Field(alias="pageSize")
    total_pages: int = Field(alias="totalPages")

    class Config:
        arbitrary_types_allowed = True
        populate_by_name = True

class AircraftDto(BaseModel):
    icao24: str
    reg: Optional[str] = None
    icaoType: Optional[str] = None
    type: Optional[str] = None
    op: Optional[str] = None
    desig: Optional[str] = None
    
    class Config:
        arbitrary_types_allowed = True
