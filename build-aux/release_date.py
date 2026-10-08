#!/usr/bin/env python3
"""Print the YYYY-MM-DD release date for the metainfo <release> entry.

Respects SOURCE_DATE_EPOCH when set (reproducible CI builds), otherwise
uses today. stdout only; fails loudly on a malformed epoch.

Usage: python3 build-aux/release_date.py
"""

from __future__ import annotations

import datetime
import os
import sys


def main() -> int:
    epoch = os.environ.get("SOURCE_DATE_EPOCH", "")
    if epoch:
        try:
            date = datetime.datetime.fromtimestamp(
                int(epoch), datetime.timezone.utc
            ).date()
        except (ValueError, OverflowError, OSError) as e:
            print(f"release_date.py: bad SOURCE_DATE_EPOCH: {e}",
                  file=sys.stderr)
            return 1
    else:
        date = datetime.date.today()
    print(date.isoformat())
    return 0


if __name__ == "__main__":
    sys.exit(main())
