// OpenFeign with native annotations.
// Click "🌐 Generate .http request" above a method, or press cmd+. on the line.
package com.example.feign;

import feign.Headers;
import feign.Param;
import feign.RequestLine;

public interface FeignRepoClient {

    @RequestLine("GET /repos/{owner}/{repo}")
    Repo get(@Param("owner") String owner, @Param("repo") String repo);

    @RequestLine("POST /repos/{owner}")
    @Headers("Content-Type: application/json")
    Repo create(@Param("owner") String owner, Repo repo);
}
