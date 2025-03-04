import time
import random
import argparse

from flask import Flask, request, jsonify


app = Flask(__name__)

@app.route('/', defaults={'path': ''}, methods=['GET', 'POST', 'PUT', 'DELETE', 'PATCH', 'HEAD'])
@app.route('/<path:path>', methods=['GET', 'POST', 'PUT', 'DELETE', 'PATCH', 'HEAD'])
def catch_all(path):
    # Simulate latency
    latency = random.uniform(0.01, 0.1)  # 10ms to 100ms
    time.sleep(latency)

    print(f"Request received at path: {path}")
    print(f"Method: {request.method}")
    print(f"Headers: {request.headers}")
    print(f"Body: {request.get_data()}")

    response_data = {
        "method": request.method,
        "path": path,
        "headers": dict(request.headers),
        "body": request.get_data(as_text=True),
        "args": request.args.to_dict()
    }
    return jsonify(response_data), 200

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description='Dummy server for testing.')
    parser.add_argument('--port', type=int, default=5000, help='Port to run the server on.')
    args = parser.parse_args()

    app.run(debug=True, port=args.port)