import http from 'k6/http';

export default function () {
      http.get('http://127.0.0.1:8080');
}
