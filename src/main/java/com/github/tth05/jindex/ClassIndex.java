package com.github.tth05.jindex;

import com.github.tth05.jindex.constantPool.ConstantPool;
import com.github.tth05.jindex.constantPool.IndexString;
import com.github.tth05.jindex.entries.IndexedClass;
import com.github.tth05.jindex.entries.IndexedPackage;
import com.github.tth05.jindex.search.PrefixTree;

import java.util.*;
import java.util.stream.Collectors;

public class ClassIndex {

    private final ConstantPool constantPool;
    private final PrefixTree<IndexedClass> classPrefixTree;
    private final PrefixTree<Integer> methodPrefixTree;

    private ClassIndex(ConstantPool constantPool, PrefixTree<IndexedClass> classPrefixTree, PrefixTree<Integer> methodPrefixTree) {
        this.constantPool = constantPool;
        this.classPrefixTree = classPrefixTree;
        this.methodPrefixTree = methodPrefixTree;
    }

    public List<IndexedClass> findClasses(IndexString className) {
        return this.classPrefixTree.findAllStartingWith(className);
    }

    public List<String> findMethods(IndexString methodName) {
        List<Integer> result = this.methodPrefixTree.findAllStartingWith(methodName);
        List<String> names = new ArrayList<>(result.size());
        for (Integer integer : result)
            names.add(this.constantPool.stringAt(integer).toJavaString());

        return names;
    }

    public ConstantPool getConstantPool() {
        return this.constantPool;
    }

    public static final class Builder {

        private final IClassInfoIterator classInfoIterator;
        private int expectedMethodCount;
        private int averageClassNameSize = 15;
        private int averageMethodNameSize = 8;

        public Builder(IClassInfoIterator classInfoIterator) {
            this.classInfoIterator = classInfoIterator;
        }

        public Builder(List<Class<?>> classes) {
            this.classInfoIterator = new IClassInfoIterator() {

                private int index;
                private Class<?> element = classes.get(0);

                @Override
                public boolean hasNext() {
                    return this.index < classes.size();
                }

                @Override
                public void advance() {
                    this.index++;
                    if (hasNext())
                        this.element = classes.get(this.index);
                }

                @Override
                public int elementCount() {
                    return classes.size();
                }

                @Override
                public IndexString currentPackageName() {
                    return new IndexString(this.element.getPackage().getName());
                }

                @Override
                public IndexString currentClassName() {
                    return new IndexString(this.element.getName()).subSequenceAfterLast((byte) '.');
                }

                @Override
                public List<IndexString> currentMethodNames() {
                    return Arrays.stream(this.element.getDeclaredMethods()).map(method -> new IndexString(method.getName())).collect(Collectors.toList());
                }
            };
        }

        public Builder setExpectedMethodCount(int expectedMethodCount) {
            this.expectedMethodCount = expectedMethodCount;
            return this;
        }

        public Builder setAverageClassNameSize(int averageClassNameSize) {
            this.averageClassNameSize = averageClassNameSize;
            return this;
        }

        public Builder setAverageMethodNameSize(int averageMethodNameSize) {
            this.averageMethodNameSize = averageMethodNameSize;
            return this;
        }

        public ClassIndex build() {
            int elementCount = this.classInfoIterator.elementCount();
            //Multiply with 0.8 to account for duplicates
            Map<IndexString, Integer> constantMap = new HashMap<>((int) ((elementCount + this.expectedMethodCount) * 0.8d));
            ConstantPool constantPool = new ConstantPool((int) ((elementCount * this.averageClassNameSize + this.expectedMethodCount * this.averageMethodNameSize) * 0.8d));

            IndexedPackage rootPackage = new IndexedPackage(0, null);
            PrefixTree<IndexedClass> classPrefixTree = new PrefixTree<>((byte) 4);
            PrefixTree<Integer> methodPrefixTree = new PrefixTree<>((byte) 4);

            //Index classes
            for (; this.classInfoIterator.hasNext(); this.classInfoIterator.advance()) {
                IndexedPackage indexedPackage = rootPackage.getOrAddPackage(constantPool, this.classInfoIterator.currentPackageName());

                List<IndexString> declaredMethods = this.classInfoIterator.currentMethodNames();
                int[] indexedMethods = new int[declaredMethods.size()];
                for (int i = 0; i < indexedMethods.length; i++) {
                    Integer cpMethodNameIndex = constantMap.computeIfAbsent(declaredMethods.get(i), constantPool::addString);
                    indexedMethods[i] = cpMethodNameIndex;
                    methodPrefixTree.put(constantPool.stringAt(cpMethodNameIndex), cpMethodNameIndex);
                }

                int cpClassNameIndex = constantMap.computeIfAbsent(this.classInfoIterator.currentClassName(), constantPool::addString);
                IndexedClass indexedClass = new IndexedClass(cpClassNameIndex, indexedPackage, indexedMethods);
                classPrefixTree.put(constantPool.stringAt(cpClassNameIndex), indexedClass);
            }

            //Free memory used by sub-packages
            Queue<IndexedPackage> queue = new ArrayDeque<>(Arrays.asList(rootPackage.getSubPackages()));
            IndexedPackage currentPackage;
            while ((currentPackage = queue.poll()) != null) {
                IndexedPackage[] subPackages = currentPackage.getSubPackages();
                if (subPackages != null)
                    queue.addAll(Arrays.asList(subPackages));
                currentPackage.freeSubPackages();
            }

            return new ClassIndex(constantPool, classPrefixTree, methodPrefixTree);
        }

        public interface IClassInfoIterator {

            boolean hasNext();

            void advance();

            int elementCount();

            IndexString currentPackageName();

            IndexString currentClassName();

            List<IndexString> currentMethodNames();
        }
    }
}
