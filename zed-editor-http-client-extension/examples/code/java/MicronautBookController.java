// Micronaut HTTP server.
// Click "🌐 Generate .http request" above a method, or press cmd+. on the line.
package com.example.micronaut;

import io.micronaut.http.annotation.*;
import io.micronaut.security.annotation.Secured;

@Controller("/books")
public class MicronautBookController {

    @Get("/{isbn}")
    public Book find(@PathVariable String isbn, @QueryValue("expand") boolean expand) { return null; }

    @Get
    public Book[] list(@QueryValue int limit) { return null; }

    @Post
    @Secured("isAuthenticated()")
    public Book save(@Header("X-Tenant") String tenant, @Body Book book) { return book; }

    @Delete("/{isbn}")
    public void remove(@PathVariable String isbn) {}
}

class Book {
    private String isbn;
    private String title;
    private int pages;
    private double price;
}
