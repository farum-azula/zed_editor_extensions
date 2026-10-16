// Axios.
// Click "🌐 Generate .http request" above a call, or press cmd+. on the line.
import axios from "axios";

interface Credentials {
  email: string;
  password: string;
}

const api = axios.create({ baseURL: process.env.API_URL });

export const login = (credentials: Credentials) => axios.post("/api/login", credentials);

export const user = (id: string) => api.get<User>(`/users/${id}`, { params: { expand: "profile" } });

export const removeUser = (id: string) =>
  api.delete(`/users/${id}`, { headers: { "X-Request-Id": "demo" } });

interface User {
  id: string;
  name: string;
  email: string;
}
