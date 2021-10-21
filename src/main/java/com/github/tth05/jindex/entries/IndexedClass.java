package com.github.tth05.jindex.entries;

import com.github.tth05.jindex.constantPool.ConstantPool;
import com.github.tth05.jindex.constantPool.IndexString;
import com.github.tth05.jindex.utils.StringUtils;

public class IndexedClass extends ConstantPoolReference {

    private final int[] methods;
    private final IndexedPackage indexedPackage;

    public IndexedClass(int cpNameIndex, IndexedPackage indexedPackage, int[] methods) {
        super(cpNameIndex);
        this.indexedPackage = indexedPackage;
        this.methods = methods;
    }

    public String findMethod(ConstantPool constantPool, IndexString methodName) {
        for (int method : this.methods) {
            if (constantPool.stringAt(method).equals(methodName))
                return getName(constantPool).toJavaString() + "#" + StringUtils.toString(methodName);
        }

        return null;
    }

    public String getFullClassName(ConstantPool constantPool) {
        return getPackageName(constantPool) + "." + getName(constantPool).toJavaString();
    }

    public String getPackageName(ConstantPool constantPool) {
        return this.indexedPackage.getFullName(constantPool);
    }

    @Override
    public String toString() {
        return "IndexedClass{cp: '" + this.cpNameIndex + "'}";
    }
}
