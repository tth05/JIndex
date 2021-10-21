package com.github.tth05.jindex.entries;

import com.github.tth05.jindex.constantPool.ConstantPool;
import com.github.tth05.jindex.utils.StringUtils;

public abstract class ConstantPoolReference {

    protected final int cpNameIndex;

    public ConstantPoolReference(int cpNameIndex) {
        this.cpNameIndex = cpNameIndex;
    }

    public ConstantPool.ConstantPoolStringView getName(ConstantPool constantPool) {
        return constantPool.stringAt(this.cpNameIndex);
    }

    public String toJavaString(ConstantPool constantPool) {
        return StringUtils.toString(getName(constantPool));
    }
}
