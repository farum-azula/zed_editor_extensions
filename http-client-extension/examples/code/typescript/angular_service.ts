// Angular HttpClient.
// Click "🌐 Generate .http request" above a call, or press cmd+. on the line.
import { HttpClient } from "@angular/common/http";
import { Injectable } from "@angular/core";

export interface Hero {
  id: number;
  name: string;
  power: string;
}

@Injectable({ providedIn: "root" })
export class HeroService {
  constructor(private http: HttpClient) {}

  list() {
    return this.http.get<Hero[]>("/api/heroes");
  }

  create(hero: Hero) {
    return this.http.post<Hero>("/api/heroes", hero, { headers: { "X-Request-Id": "demo" } });
  }

  remove(id: number) {
    return this.http.delete<void>(`/api/heroes/${id}`);
  }
}
