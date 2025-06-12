package cn.itcraft.bbpe4j;

import java.util.concurrent.CompletableFuture;

/**
 * @author Helly Guo
 * <p>
 * Created on 11/21/23 10:57 AM
 */
class WrappedData<T> {
    private final CompletableFuture<Boolean> future;
    private final int id;
    private final T data;

    public WrappedData(CompletableFuture<Boolean> future, int id, T data) {
        this.future = future;
        this.id = id;
        this.data = data;
    }

    public CompletableFuture<Boolean> getFuture() {
        return future;
    }

    public int getId() {
        return id;
    }

    public T getData() {
        return data;
    }
}
