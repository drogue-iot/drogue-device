#!/usr/bin/env python3

from http.server import BaseHTTPRequestHandler, HTTPServer
import json
import logging
import ssl

class S(BaseHTTPRequestHandler):
    def do_POST(self):
        logging.info("POST")
        content_length = int(self.headers['Content-Length'])
        post_data = self.rfile.read(content_length)
        logging.info("POST request,\nPath: %s\nHeaders:\n%s\n\nBody:\n%s\n",
                str(self.path), str(self.headers), post_data.decode('utf-8'))

        #data = [k for k in range(1, 200)]
        payload = post_data #json.dumps(data).encode()

        self.send_response(200)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', len(payload))
        self.end_headers()
        self.wfile.write(payload)

def run(server_class=HTTPServer, handler_class=S, port=8088):
    logging.basicConfig(level=logging.DEBUG)
    server_address = ('10.10.10.4', port)
    httpd = server_class(server_address, handler_class)
    context = ssl.create_default_context();
    context.minimum_version = ssl.TLSVersion.TLSv1_3
    context.maximum_version = ssl.TLSVersion.TLSv1_3
    context.check_hostname = False
    context.load_cert_chain('server-cert.pem', 'server-key.pem')
    httpd.socket = context.wrap_socket(httpd.socket, server_side=True)
    logging.info('Starting httpd...\n')
    try:
        httpd.serve_forever()
    except KeyboardInterrupt:
        pass
    httpd.server_close()
    logging.info('Stopping httpd...\n')

run(port=8088)
