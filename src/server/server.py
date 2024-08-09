from flask import Flask, request, jsonify
import subprocess
import os

app = Flask(__name__)

@app.route('/run-rover', methods=['POST'])
def run_rover():
    try:
        result = subprocess.run(['/dist/rover', 'supergraph', 'compose', '--output', '/dist/schema.graphql', '--config', '/dist/rover-config.yaml'], capture_output=True, text=True)
        return jsonify({
            'stdout': result.stdout,
            'stderr': result.stderr,
            'returncode': result.returncode
        })
    except Exception as e:
        return jsonify({'error': str(e)}), 500

@app.route('/add-sub-schema', methods=['POST'])
def save_schema():
    try:
        # Get the schema string from the request body
        sub_schema = request.json.get('schema', '')
        module = request.json.get('module', 'webui')

        if not sub_schema:
            return jsonify({'error': 'Schema string is required'}), 400

        # Path to save the schema file
        schema_file_path = '/dist/' + module +'.graphql'

        # Ensure the directory exists
        os.makedirs(os.path.dirname(schema_file_path), exist_ok=True)

        # Save the schema string to the file
        with open(schema_file_path, 'w') as schema_file:
            schema_file.write(sub_schema)

        return jsonify({'message': f'Schema saved to {schema_file_path}'}), 200

    except Exception as e:
        return jsonify({'error': str(e)}), 500

if __name__ == '__main__':
    app.run(host='0.0.0.0', port=8080)
