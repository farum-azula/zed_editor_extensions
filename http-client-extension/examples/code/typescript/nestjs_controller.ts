// NestJS.
// Click "🌐 Generate .http request" above a route, or press cmd+. on the line.
import { Body, Controller, Delete, Get, Headers, Post, Query, UseGuards } from "@nestjs/common";

class CreateCatDto {
  name: string;
  age: number;
  breed: string;
}

@Controller("cats")
export class CatsController {
  @Get(":id")
  findOne(@Query("expand") expand: string) {
    return {};
  }

  @Get()
  findAll(@Query("limit") limit: number) {
    return [];
  }

  @Post()
  @UseGuards(AuthGuard)
  create(@Headers("X-Request-Id") rid: string, @Body() dto: CreateCatDto) {
    return dto;
  }

  @Delete(":id")
  remove() {}
}
