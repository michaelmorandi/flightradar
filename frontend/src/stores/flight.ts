import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import { Flight } from '@/model/backendModel';
import { FlightRadarService } from '@/services/flightRadarService';

export const useFlightStore = defineStore('flight', () => {
  // State
  const flights = ref<Flight[]>([]);
  const selectedFlight = ref<Flight | null>(null);
  const selectedFlightId = ref<string | null>(null);
  const loading = ref(false);
  const error = ref<Error | null>(null);
  const searchFilter = ref<string>('');
  const numEntries = ref<number>(50);

  // Getters
  const filteredFlights = computed(() => {
    if (!searchFilter.value) return flights.value;
    
    const filter = searchFilter.value.toLowerCase();
    return flights.value.filter(flight => 
      flight.id?.toLowerCase().includes(filter) ||
      flight.icao24?.toLowerCase().includes(filter) ||
      flight.cls?.toLowerCase().includes(filter)
    );
  });

  const isFlightSelected = computed(() => selectedFlightId.value !== null);

  const getFlightById = computed(() => {
    return (id: string) => flights.value.find(flight => flight.id === id);
  });

  // Actions
  const fetchFlights = async (radarService: FlightRadarService, entries?: number, filter?: string) => {
    loading.value = true;
    error.value = null;
    
    try {
      const entriesCount = entries ?? numEntries.value;
      const searchQuery = filter ?? searchFilter.value;
      
      radarService.getFlights(entriesCount, searchQuery).subscribe({
        next: (flightData: Flight[]) => {
          flights.value = flightData;
          loading.value = false;
        },
        error: (err: Error) => {
          error.value = err;
          loading.value = false;
          console.error('Error fetching flights:', err);
        }
      });
    } catch (err) {
      error.value = err as Error;
      loading.value = false;
      console.error('Error setting up flight subscription:', err);
    }
  };

  const fetchFlightDetails = async (radarService: FlightRadarService, flightId: string) => {
    loading.value = true;
    error.value = null;
    
    try {
      radarService.getFlight(flightId).subscribe({
        next: (flight: Flight) => {
          selectedFlight.value = flight;
          selectedFlightId.value = flightId;
          loading.value = false;
        },
        error: (err: Error) => {
          error.value = err;
          loading.value = false;
          console.error('Error fetching flight details:', err);
        }
      });
    } catch (err) {
      error.value = err as Error;
      loading.value = false;
      console.error('Error setting up flight details subscription:', err);
    }
  };

  const selectFlight = (flightId: string | null) => {
    selectedFlightId.value = flightId;
    if (flightId === null) {
      selectedFlight.value = null;
    }
  };

  const updateSearchFilter = (filter: string) => {
    searchFilter.value = filter;
  };

  const updateNumEntries = (entries: number) => {
    numEntries.value = entries;
  };

  const clearError = () => {
    error.value = null;
  };

  const clearSelectedFlight = () => {
    selectedFlight.value = null;
    selectedFlightId.value = null;
  };

  const clearFlights = () => {
    flights.value = [];
  };

  // Reset store state
  const $reset = () => {
    flights.value = [];
    selectedFlight.value = null;
    selectedFlightId.value = null;
    loading.value = false;
    error.value = null;
    searchFilter.value = '';
    numEntries.value = 50;
  };

  return {
    // State
    flights,
    selectedFlight,
    selectedFlightId,
    loading,
    error,
    searchFilter,
    numEntries,
    
    // Getters
    filteredFlights,
    isFlightSelected,
    getFlightById,
    
    // Actions
    fetchFlights,
    fetchFlightDetails,
    selectFlight,
    updateSearchFilter,
    updateNumEntries,
    clearError,
    clearSelectedFlight,
    clearFlights,
    $reset
  };
});