package com.erayt.bbpe4j;

import org.jctools.queues.MessagePassingQueue;
import org.jctools.queues.atomic.MpscAtomicArrayQueue;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.concurrent.atomic.AtomicBoolean;

/**
 * @author Helly Guo
 * <p>
 * Created on 11/21/23 10:51 AM
 */
class JavaBbpeThread {

    private static final Logger LOGGER = LoggerFactory.getLogger(JavaBbpeThread.class);

    private final AtomicBoolean active = new AtomicBoolean(true);
    private final Thread thread;
    private final MpscAtomicArrayQueue<WrappedData<?>> queue = new MpscAtomicArrayQueue<>(10240);

    public JavaBbpeThread() {
        this.thread = new Thread(this::runBbpe, "java-bbpe-thread");
        this.thread.setDaemon(true);
    }

    public void fillQueue(WrappedData<?> data) {
        queue.offer(data);
    }

    private void runBbpe() {
        MessagePassingQueue.Consumer<WrappedData<?>> dataConsumer = this::consumeData;
        MessagePassingQueue.WaitStrategy waitStrategy = new AdaptiveWaitStrategy();
        MessagePassingQueue.ExitCondition exitCondition = active::get;
        while (active.get()) {
            queue.drain(dataConsumer, waitStrategy, exitCondition);
        }
    }

    private <T> void consumeData(WrappedData<T> data) {
        boolean success = JavaBbpe.newData(data.getId(), data.getData());
        data.getFuture().complete(success);
    }

    public void start() {
        this.thread.start();
    }

    public void stop(long timeout) {
        active.set(false);
        try {
            this.thread.join(timeout);
        } catch (InterruptedException e) {
            LOGGER.warn("interrupted", e);
        }
    }
}
