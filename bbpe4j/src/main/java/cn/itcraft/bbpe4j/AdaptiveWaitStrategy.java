package cn.itcraft.bbpe4j;

import org.jctools.queues.MessagePassingQueue;

import java.util.concurrent.locks.LockSupport;

/**
 * @author Helly Guo
 * <p>
 * Created on 2025-05-30 15:15
 */
public class AdaptiveWaitStrategy implements MessagePassingQueue.WaitStrategy {

    private static final int SPIN_THRESHOLD = 100;
    private static final int YIELD_THRESHOLD = 1000;
    // 1ms
    private static final long MAX_PARK_NANOS = 1_000_000;

    private int parkCount = 0;

    @Override
    public int idle(int counter) {
        if (counter < SPIN_THRESHOLD) {
            // 短期等待：忙等待
            return counter + 1;
        } else if (counter < YIELD_THRESHOLD) {
            // 中期等待：线程让步
            Thread.yield();
            return counter + 1;
        } else {
            // 长期等待：parkNanos 并指数退避
            long parkTime = Math.min(MAX_PARK_NANOS, 100L * (1L << parkCount));
            LockSupport.parkNanos(parkTime);
            // 指数退避但不超过最大值
            parkCount = Math.min(parkCount + 1, 16);
            // 重置计数器
            return 0;
        }
    }
}
