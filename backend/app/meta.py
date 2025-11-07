import json
from pathlib import Path

class MetaInformation:
    """Loads build metadata from resources/meta.json (generated at build time)"""

    def __init__(self):
        self.commit_id = "unknown"
        self.build_timestamp = "unknown"

        meta_file = Path('resources/meta.json')
        if meta_file.is_file():
            with open(meta_file, 'r') as f:
                data = json.load(f)
                self.commit_id = data.get('commit_id', 'unknown')
                self.build_timestamp = data.get('build_timestamp', 'unknown')