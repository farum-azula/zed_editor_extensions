// Spring HTTP service clients (@HttpExchange interfaces, Spring Framework 6+).
// Click "🌐 Generate .http request" above an exchange, or press cmd+. on the line.
// The request goes where the client's group does: the group of the
// @ImportHttpServices that lists the interface (in this file or anywhere in the
// project), and its spring.http.serviceclient.<group>.base-url from
// application.yml / application.properties (else a {{catalogBaseUrl}}-style
// variable). The interface's @HttpExchange is the prefix.
package com.example.clients;

import java.util.List;
import org.springframework.context.annotation.Configuration;
import org.springframework.web.bind.annotation.*;
import org.springframework.web.multipart.MultipartFile;
import org.springframework.web.service.annotation.*;
import org.springframework.web.service.registry.ImportHttpServices;

@HttpExchange("/books")
public interface SpringHttpServiceClient {

    @GetExchange("/{isbn}")
    Book find(@PathVariable String isbn);

    @GetExchange
    List<Book> search(@RequestParam("q") String query, @RequestParam(required = false) Integer year);

    @PostExchange
    Book create(@RequestHeader("X-Request-Id") String requestId, @CookieValue("session") String session,
                @RequestBody Book book);

    @PutExchange(url = "/{isbn}/cover", contentType = "multipart/form-data")
    void uploadCover(@PathVariable String isbn, @RequestPart("file") MultipartFile file);

    @DeleteExchange("/{isbn}")
    void remove(@PathVariable String isbn);

    record Book(String isbn, String title, int stock) {}
}

@Configuration
@ImportHttpServices(group = "catalog", types = SpringHttpServiceClient.class)
class ClientConfig {}
