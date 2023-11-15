from abc import abstractmethod, ABCMeta
import shower4py


class ShowerRecordCallback(metaclass=ABCMeta):
    """Abstract class for Shower record callback."""

    @abstractmethod
    def callback(self, data) -> None:
        """callback. 回调

        Args:
            data(long[]): 数据.
        """


class Shower:
    """Shower client."""

    @staticmethod
    def start() -> bool:
        """Start Shower client. 启动

        Returns:
            bool: True if start success. 是否成功
        """
        return shower4py.start()

    @staticmethod
    def stop() -> bool:
        """Stop Shower client. 停止

        Returns:
            bool: True if stop success. 是否成功
        """
        return shower4py.stop()
    
    @staticmethod
    def def_incoming(name:str, names:list,types:list,lengths:list)->int:
        """Define incoming var name. 通过 name 定义输入

        Args:
            name (str): name. 名称
            names (list): names. 名称列表
            types (list): types. 类型列表
            lengths (list): lengths. 长度列表

        Returns:
            int: id. 标号
        """
        return shower4py.def_incoming(name, names, types, lengths)
    
    @staticmethod
    def def_stream(name:str, names:list,types:list,lengths:list)->int:
        """Define stream var name. 通过 name 定义流

        Args:
            name (str): name. 名称
            names (list): names. 名称列表
            types (list): types. 类型列表
            lengths (list): lengths. 长度列表

        Returns:
            int: id. 标号
        """
        return shower4py.def_stream(name, names, types, lengths)

    @staticmethod
    def new_data(id: int, bdata: bytes) -> bool:
        """Create table. 新建数据表

        Args:
            id (int): id. 标号
            bdata (bytes): data. 数据

        Returns:
            bool: True if create success. 是否成功
        """
        return shower4py.new_data(id, bdata)

    @staticmethod
    def def_mapper(sql:str, callback:ShowerRecordCallback)-> bool:
        """Define mapper var sql. 通过 sql 定义动作

        Args:
            sql (str): sql statement. sql 语句。
            callback (ShowerRecordCallback): callback function. 回调函数

        Returns:
            int: >=0 if define success. 是否成功
        """
        return shower4py.def_mapper(sql, callback)

    @staticmethod
    def def_mapper_bind_aggregate(sql:str, aggregate_id:int)-> int:
        """Define mapper var sql. 通过 sql 定义动作

        Args:
            sql (str): sql statement. sql 语句。
            callback (ShowerRecordCallback): callback function. 回调函数

        Returns:
            int: >=0 if define success. 是否成功
        """
        return shower4py.def_mapper_bind_aggregate(sql, aggregate_id)

    @staticmethod
    def def_aggregate(sql:str, callback:ShowerRecordCallback)-> int:
        """Define aggregate var sql. 通过 sql 定义动作

        Args:
            sql (str): sql statement. sql 语句。
            callback (ShowerRecordCallback): callback function. 回调函数

        Returns:
            int: >=0 if define success. 是否成功
        """
        return shower4py.def_aggregate(sql, callback)
