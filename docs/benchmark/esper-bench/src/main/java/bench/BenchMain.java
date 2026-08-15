package bench;

import com.espertech.esper.client.Configuration;
import com.espertech.esper.client.EPServiceProvider;
import com.espertech.esper.client.EPServiceProviderManager;
import com.espertech.esper.client.EPStatement;
import com.espertech.esper.client.EventBean;
import com.espertech.esper.client.UpdateListener;

import java.util.HashMap;
import java.util.Map;

/** Esper 7.1 benchmark: same scenarios as BPE (filter / computed fields / window aggregate). */
public class BenchMain {
    static final int N = 1_000_000;
    static final long BASE_TS = 1_700_000_000_000L;

    static void run(String name, Runnable send, int n) {
        for (int i = 0; i < 100_000; i++) send.run(); // warmup (EPL compiled, JIT warmed)
        System.gc();
        long start = System.nanoTime();
        for (int i = 0; i < n; i++) send.run();
        long el = System.nanoTime() - start;
        double ns = el / (double) n;
        System.out.printf("esper %-28s %10.1f ns/op  %12.0f events/s%n", name, ns, 1e9 / ns);
    }

    static Map<String, Object> makeEvent(long i) {
        Map<String, Object> e = new HashMap<>();
        e.put("ts", BASE_TS + i);
        e.put("user_id", i % 1000);
        e.put("amount", 1000L + (i % 100));
        e.put("risk", 1L);
        e.put("a", 1L + (i % 50));
        e.put("b", 1L + (i % 50));
        e.put("c", 1L + (i % 50));
        e.put("d", 1L + (i % 50));
        return e;
    }

    public static void main(String[] args) {
        Configuration config = new Configuration();
        Map<String, Object> types = new HashMap<>();
        types.put("ts", Long.class);
        types.put("user_id", Long.class);
        types.put("amount", Long.class);
        types.put("risk", Long.class);
        types.put("a", Long.class);
        types.put("b", Long.class);
        types.put("c", Long.class);
        types.put("d", Long.class);
        config.addEventType("Txn", types);
        EPServiceProvider ep = EPServiceProviderManager.getDefaultProvider(config);
        UpdateListener empty = new UpdateListener() {
            public void update(EventBean[] newEvents, EventBean[] oldEvents) {}
        };

        // 1) pure filter, 6 conditions (matches BPE perf_diff WHERE)
        EPStatement s1 = ep.getEPAdministrator()
            .createEPL("select * from Txn where amount > 100 and risk = 1 and a > 0 and b > 0 and c > 0 and d > 0");
        s1.addListener(empty);
        Map<String, Object> ev = makeEvent(0);
        final long[] i = {0};
        run("filter-6cond", () -> { i[0]++; ev.put("amount", 1000L + (i[0] % 100)); ep.getEPRuntime().sendEvent(ev, "Txn"); }, N);
        s1.destroy();

        // 2) filter + 5 computed fields (matches BPE perf_diff SELECT)
        EPStatement s2 = ep.getEPAdministrator()
            .createEPL("select user_id, amount*1.0, a+b, c-d, amount*2 from Txn where amount > 100");
        s2.addListener(empty);
        run("filter+5fields", () -> { i[0]++; ev.put("amount", 1000L + (i[0] % 100)); ep.getEPRuntime().sendEvent(ev, "Txn"); }, N);
        s2.destroy();

        // 3) window aggregate: length_batch(10), sum/count/avg (matches BPE perf_agg LIMIT 10)
        EPStatement s3 = ep.getEPAdministrator()
            .createEPL("select sum(amount) as s, count(*) as c, avg(amount) as a from Txn.win:length_batch(10)");
        s3.addListener(empty);
        run("window-agg(len_batch10)", () -> { i[0]++; ev.put("amount", 1000L + (i[0] % 100)); ep.getEPRuntime().sendEvent(ev, "Txn"); }, N);
        s3.destroy();

        // 3b) length(10) per-event window aggregate (matches BPE bind-aggregate LIMIT 10)
        EPStatement s3b = ep.getEPAdministrator()
            .createEPL("select sum(amount) as s, count(*) as c from Txn.win:length(10)");
        s3b.addListener(empty);
        run("window-agg(len10)", () -> { i[0]++; ev.put("amount", 1000L + (i[0] % 100)); ep.getEPRuntime().sendEvent(ev, "Txn"); }, N);
        s3b.destroy();

        // 4) length(2048) sliding window per-event aggregate (matches BPE window-full aggregation)
        EPStatement s4 = ep.getEPAdministrator()
            .createEPL("select sum(amount) as s, count(*) as c from Txn.win:length(2048)");
        s4.addListener(empty);
        run("window-agg(len2048)", () -> { i[0]++; ev.put("amount", 1000L + (i[0] % 100)); ep.getEPRuntime().sendEvent(ev, "Txn"); }, N / 4);
        s4.destroy();

        ep.destroy();
    }
}
