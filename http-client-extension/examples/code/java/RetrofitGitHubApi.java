// Retrofit (Java).
// Click "🌐 Generate .http request" above a method, or press cmd+. on the line.
package com.example.retrofit;

import java.util.List;
import retrofit2.Call;
import retrofit2.http.*;

public interface RetrofitGitHubApi {

    @GET("users/{user}/repos")
    Call<List<Repo>> repos(@Path("user") String user, @Query("sort") String sort);

    @POST("user/repos")
    Call<Repo> create(@Header("Authorization") String auth, @Body Repo repo);

    @DELETE("repos/{owner}/{name}")
    Call<Void> delete(@Path("owner") String owner, @Path("name") String name);
}

class Repo {
    String name;
    String description;
    boolean privateRepo;
}
