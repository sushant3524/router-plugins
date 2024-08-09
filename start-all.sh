#!/bin/bash

# Start the router in the background
/dist/start.sh &

# Start the Flask server
python3 /dist/server.py
