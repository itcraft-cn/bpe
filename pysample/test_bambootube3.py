import os
from pybambootube.bambootube import BambooTube, BambooTubeRecordCallback
from time import sleep

SQL = "select demo.a from demo limit 10"


class XDemo(BambooTubeRecordCallback):
    def __init__(self):
        super().__init__()

    def callback(self, data):
        print(data)


class TestBambooTube:
    # 函数级开始
    def setup_method(self):
        os.environ["BAMBOOTUBE_HOME"] = "/home/helly/code/rust/bambootube"
        BambooTube.start()

    # 函数级结束
    def teardown_method(self):
        BambooTube.stop()

    # 测试
    def test(self):
        callback = XDemo()
        BambooTube.def_incoming("demo", ["a"], [0], [0])
        global SQL
        mapper_id = BambooTube.def_mapper(SQL, callback)
        print("mapper_id:", mapper_id)
        futures = list()
        for i in range(100):
            futures.append(BambooTube.new_data_async(1, b"100000000000000000000000"))
        while True:
            if all(future.done() for future in futures):
                break
            else:
                sleep(0.1)


if __name__ == "__main__":
    a = TestBambooTube()
    a.setup_method()
    a.test()
    a.teardown_method()
